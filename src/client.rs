use pyo3::prelude::*;
use pyo3::exceptions::PyRuntimeError;
use futures::TryStreamExt;
use noodles::htsget;
use crate::parse::{parse_bam_records, parse_cram_records};
use crate::RecordIter;

fn fetch_bytes(
    base_url: &str,
    id: &str,
    format: &str,
    region: Option<&str>,
) -> Result<Vec<u8>, std::io::Error> {
    let fmt = match format {
        "CRAM" => htsget::reads::Format::Cram,
        "BAM" => htsget::reads::Format::Bam,
        other => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("unsupported format: {other}"),
            ))
        }
    };
    let url = base_url
        .parse()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("{e}")))?;
    let client = htsget::Client::new(url);
    let mut request = client.reads(id).set_format(fmt);
    if let Some(r) = region {
        request = request.add_region(
            r.parse()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("{e}")))?,
        );
    }
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let response = request
                .send()
                .await
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?;
            let mut chunks = response.chunks();
            let mut buf = Vec::new();
            while let Some(chunk) = chunks
                .try_next()
                .await
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))?
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
    let data = fetch_bytes(base_url, id, format, region)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    let records = match format {
        "CRAM" => parse_cram_records(&data),
        "BAM" => parse_bam_records(&data),
        other => return Err(PyRuntimeError::new_err(format!("unsupported format: {other}"))),
    }
    .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    Ok(RecordIter { records, index: 0 })
}
