use async_stream::try_stream;
use std::pin::Pin;
use futures::Stream;
use pyo3::prelude::*;
use io::Error;
use futures::{StreamExt, TryStreamExt};
use noodles::{
    core::{region::Interval, Region},
    cram,
    cram::io::reader::Reader,
    fasta,
    htsget as htsget,
    sam,
};
use tokio::io::{self, AsyncWriteExt};
use tokio_util::io::StreamReader;

// e.g. stream("http://localhost/htsget", "dir/file", "chr1:123-4541")
//#[pyfunction]
//fn py_stream(py: Python, base_url: String, id: String, region: String, fasta_src: String) -> PyResult<Bound<PyAny>> {
//    pyo3_async_runtimes::tokio::future_into_py(py, async move {
//        stream(&base_url, &id, &region, &fasta_src).await.map_err(|e| {
//            pyo3::exceptions::PyRuntimeError::new_err(format!("Error in Rust function: {}", e))
//        })
//    })
//}

pub async fn stream(base_url: &str, id: &str, region: &str, fasta_src: &str) -> Result<(), Error> {
    
    // Initialize request parameters
    let format = htsget::reads::Format::Cram;
    let region: Region = region.parse().expect("invalid region");
    let url = base_url.parse().unwrap();
    let client = htsget::Client::new(url);

    // Prepare reference sequence repository
    let ref_seq_repo = fasta::io::indexed_reader::Builder::default()
        .build_from_path(fasta_src)
        .map(fasta::repository::adapters::IndexedReader::new)
        .map(fasta::Repository::new)?;

    // TODO: generic on variants and reads
    // Build request
    let request = client
        .reads(id)
        .set_format(format)
        .add_region(region.clone());

    // Send request
    let reads = request.send().await.expect("request failed");

    // Prepare chunks
    let chunks = Box::pin(
        reads
            .chunks()
            .map(|result| result.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)))
    );

    let chunks_reader = StreamReader::new(chunks);

    let mut reader = cram::r#async::io::reader::Builder::default()
        .set_reference_sequence_repository(ref_seq_repo)
        .build_from_reader(chunks_reader);

    let header = reader.read_header().await?;

    let mut records = reader.records(&header);

    let mut writer = sam::r#async::io::Writer::new(io::stdout());
    writer.write_header(&header).await?;

    let ref_seq_id = header
        .reference_sequences()
        .get_index_of(region.name())
        .expect("missing reference sequence");

    let region_interval = region.interval();

    while let Some(record) = records.try_next().await? {

        if !intersects(&record, ref_seq_id, region_interval) {
            continue;
        }
        writer.write_alignment_record(&header, &record).await?;
    }

    Ok(())
}

fn stream_records<'a>(
    base_url: &'a str,
    id: &'a str,
    region: &'a str,
    fasta_src: &'a str,
) -> Pin<Box<dyn Stream<Item = Result<(), io::Error>> + Send>> {
    Box::pin(try_stream! {
        let format = htsget::reads::Format::Cram;
        let region: Region = region.parse().expect("invalid region");
        let url = base_url.parse().unwrap();
        let client = htsget::Client::new(url);

        let ref_seq_repo = fasta::io::indexed_reader::Builder::default()
            .build_from_path(fasta_src)
            .map(fasta::repository::adapters::IndexedReader::new)
            .map(fasta::Repository::new)?;

        let request = client
            .reads(id)
            .set_format(format)
            .add_region(region.clone());

        let reads = request.send().await.expect("request failed");

        let chunks = reads
            .chunks()
            .map(|result| result.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)));

        let chunks_reader = StreamReader::new(chunks);

        let mut reader = cram::r#async::io::reader::Builder::default()
            .set_reference_sequence_repository(ref_seq_repo)
            .build_from_reader(chunks_reader);

        let header = reader.read_header().await?;
        let mut records = reader.records(&header);

        let ref_seq_id = header
            .reference_sequences()
            .get_index_of(region.name())
            .expect("missing reference sequence");

        let region_interval = region.interval();

        while let Some(record) = records.try_next().await? {
            if intersects(&record, ref_seq_id, region_interval) {
                println!("{:?}", record);
                yield 
            }
        }
    })
}

fn intersects(record: &cram::Record, ref_seq_id: usize, interval: Interval) -> bool {
    if let (Some(id), Some(start), Some(end)) = (
            record.reference_sequence_id(),
            record.alignment_start(),
            record.alignment_end(),
        ) {
            id == ref_seq_id && interval.intersects((start..=end).into())
    } else {
        false
    }
}


//#[pymodule]
//pub fn client(m: &Bound<'_, PyModule>) -> PyResult<()> {
//    m.add_function(wrap_pyfunction!(stream, m)?)?;
//    Ok(())
//}

/// Test client streaming. Assumes an htsget server is already running on localhost

#[cfg(test)]
pub mod tests {
    use super::*;
    use testcontainers::{
        core::{
            IntoContainerPort,
            Mount,
            logs::LogFrame,
        },
        runners::SyncRunner,
        GenericImage, ImageExt,
    };


    #[test]
    fn test_stream() -> Result<(), Error>{
 
        // Spin up testcontainer with htsget server
        let _server = GenericImage::new("ghcr.io/umccr/htsget-rs", "dev-94")
          .with_mapped_port(8080, 8080.tcp())
          .with_mapped_port(8081, 8081.tcp())
          .with_mount(
            Mount::bind_mount(
                std::fs::canonicalize("./data")?
                    .to_string_lossy(),
                "/data"
            )
          )
        .with_log_consumer(move |frame: &LogFrame| {
            println!("{}", String::from_utf8_lossy(frame.bytes()));
        })
          .start()
          .unwrap();

        // Test client: stream from server
        use tokio::runtime::Runtime;
        let runtime = Runtime::new().expect("Failed to create tokio runtime");
        runtime.block_on(async {
            let base_url = "http://localhost:8080/reads";
            let id = "data/cram/htsnexus_test_NA12878";
            let region = "11:4900000-5000000";
            let ref_seq = "/data/genomic/e_coli/ecoli_sample.fa";
            let result = stream(base_url, id, region, ref_seq).await;
            assert!(result.is_ok(), "Stream function failed: {:?}", result);
        });

        Ok(())
    }
}
