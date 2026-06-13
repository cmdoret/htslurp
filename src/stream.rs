use std::io;
use std::path::{Path, PathBuf};
use std::thread;

use futures::{StreamExt, TryStreamExt};
use noodles::{bam, core::Region, cram, fasta, htsget, sam};
use tokio::runtime::Builder as TokioBuilder;
use tokio::sync::mpsc;
use tokio_util::io::StreamReader;

use crate::parse::{header_to_sam_bytes, record_to_sam_bytes, RegionFilter};

const BUFFER: usize = 256;

pub type RecordRx = mpsc::Receiver<io::Result<Vec<u8>>>;

/// Spawn a worker thread that fetches and decodes alignments from htsget,
/// returning the SAM header bytes and a receiver that yields one SAM-format
/// record per recv. The channel closes when the stream is exhausted; an
/// `Err` item indicates a mid-stream decode/transport error.
pub fn start_stream(
    base_url: String,
    id: String,
    format: htsget::reads::Format,
    region: Option<Region>,
    reference: Option<PathBuf>,
) -> io::Result<(Vec<u8>, RecordRx)> {
    let (tx, mut rx) = mpsc::channel::<io::Result<Vec<u8>>>(BUFFER);

    thread::spawn(move || {
        let runtime = match TokioBuilder::new_current_thread().enable_all().build() {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.blocking_send(Err(e));
                return;
            }
        };
        runtime.block_on(async move {
            if let Err(e) = run(base_url, id, format, region, reference, &tx).await {
                let _ = tx.send(Err(io::Error::other(e.to_string()))).await;
            }
        });
    });

    // The first message must be the header (or an initialization error).
    match rx.blocking_recv() {
        Some(Ok(h)) => Ok((h, rx)),
        Some(Err(e)) => Err(e),
        None => Err(io::Error::other(
            "worker thread closed channel before sending header",
        )),
    }
}

async fn run(
    base_url: String,
    id: String,
    format: htsget::reads::Format,
    region: Option<Region>,
    reference: Option<PathBuf>,
    tx: &mpsc::Sender<io::Result<Vec<u8>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let url = base_url.parse()?;
    let client = htsget::Client::new(url);
    let mut request = client.reads(&id).set_format(format);
    if let Some(r) = &region {
        request = request.add_region(r.clone());
    }
    let response = request.send().await?;

    // chunks() borrows from response, so response must live for the whole call.
    let chunks = response
        .chunks()
        .map_err(|e| io::Error::other(e.to_string()));
    let async_read = StreamReader::new(chunks);

    match format {
        htsget::reads::Format::Bam => stream_bam(async_read, region.as_ref(), tx).await,
        htsget::reads::Format::Cram => {
            stream_cram(async_read, reference.as_deref(), region.as_ref(), tx).await
        }
    }
}

// Decode each record, drop those outside `filter`, and forward the SAM-encoded
// bytes. The BAM and CRAM paths differ only in reader setup; this is their
// shared loop.
async fn forward_records<S, R>(
    mut records: S,
    header: &sam::Header,
    filter: Option<&RegionFilter>,
    tx: &mpsc::Sender<io::Result<Vec<u8>>>,
) -> Result<(), Box<dyn std::error::Error>>
where
    S: futures::Stream<Item = io::Result<R>> + Unpin,
    R: sam::alignment::Record,
{
    while let Some(record) = records.next().await {
        // A decode error is terminal: the binary stream is likely desynced, so
        // we surface the error (it reaches Python as a raised exception) and
        // stop rather than emit garbage from later offsets.
        let record = record?;
        if let Some(filter) = filter {
            if !filter.matches(&record, header) {
                continue;
            }
        }
        let bytes = record_to_sam_bytes(header, &record)?;
        if tx.send(Ok(bytes)).await.is_err() {
            return Ok(());
        }
    }
    Ok(())
}

async fn stream_bam<R>(
    inner: R,
    region: Option<&Region>,
    tx: &mpsc::Sender<io::Result<Vec<u8>>>,
) -> Result<(), Box<dyn std::error::Error>>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut reader = bam::r#async::io::Reader::new(inner);
    let header = reader.read_header().await?;
    if tx.send(Ok(header_to_sam_bytes(&header)?)).await.is_err() {
        return Ok(());
    }

    let filter = region.map(|r| RegionFilter::new(&header, r));
    forward_records(reader.records(), &header, filter.as_ref(), tx).await
}

async fn stream_cram<R>(
    inner: R,
    reference: Option<&Path>,
    region: Option<&Region>,
    tx: &mpsc::Sender<io::Result<Vec<u8>>>,
) -> Result<(), Box<dyn std::error::Error>>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let repo = match reference {
        Some(path) => fasta::io::indexed_reader::Builder::default()
            .build_from_path(path)
            .map(fasta::repository::adapters::IndexedReader::new)
            .map(fasta::Repository::new)?,
        None => fasta::Repository::default(),
    };
    let mut reader = cram::r#async::io::Reader::new(inner);
    reader.read_file_definition().await?;
    let header_text = reader.read_file_header().await?;
    let header: sam::Header = header_text.parse()?;
    if tx.send(Ok(header_to_sam_bytes(&header)?)).await.is_err() {
        return Ok(());
    }

    let filter = region.map(|r| RegionFilter::new(&header, r));
    forward_records(reader.records(&repo, &header), &header, filter.as_ref(), tx).await
}
