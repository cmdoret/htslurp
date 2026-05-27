use pyo3::prelude::*;
use pyo3::exceptions::PyRuntimeError;
use futures::TryStreamExt;
use noodles::htsget;
use crate::parse::{parse_bam_records, parse_cram_records};
use crate::RecordIter;

type ParseFn = fn(&[u8]) -> Result<Vec<Vec<u8>>, Box<dyn std::error::Error>>;

fn io_err(kind: std::io::ErrorKind, e: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(kind, e.to_string())
}

fn fetch_bytes(
    base_url: &str,
    id: &str,
    format: htsget::reads::Format,
    region: Option<&str>,
) -> Result<Vec<u8>, std::io::Error> {
    let url = base_url
        .parse()
        .map_err(|e| io_err(std::io::ErrorKind::InvalidInput, e))?;
    let client = htsget::Client::new(url);
    let mut request = client.reads(id).set_format(format);
    if let Some(r) = region {
        request = request.add_region(
            r.parse().map_err(|e| io_err(std::io::ErrorKind::InvalidInput, e))?,
        );
    }
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let response = request
                .send()
                .await
                .map_err(|e| io_err(std::io::ErrorKind::Other, e))?;
            let mut chunks = response.chunks();
            let mut buf = Vec::new();
            while let Some(chunk) = chunks
                .try_next()
                .await
                .map_err(|e| io_err(std::io::ErrorKind::Other, e))?
            {
                buf.extend_from_slice(&chunk);
            }
            Ok(buf)
        })
}

#[pyfunction]
#[pyo3(signature = (base_url, id, format, region=None))]
pub fn stream_records(
    base_url: &str,
    id: &str,
    format: &str,
    region: Option<&str>,
) -> PyResult<RecordIter> {
    let (fmt, parser): (htsget::reads::Format, ParseFn) = match format {
        "CRAM" => (htsget::reads::Format::Cram, parse_cram_records),
        "BAM" => (htsget::reads::Format::Bam, parse_bam_records),
        other => return Err(PyRuntimeError::new_err(format!("unsupported format: {other}"))),
    };
    let data = fetch_bytes(base_url, id, fmt, region)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    let records = parser(&data).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    Ok(RecordIter { records, index: 0 })
}
