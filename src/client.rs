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

#[cfg(test)]
mod tests {
    use super::*;
    use testcontainers::{
        core::{IntoContainerPort, Mount},
        runners::SyncRunner,
        GenericImage, ImageExt,
    };

    // Requires Docker. Run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn cram_round_trip_against_htsget_rs() {
        let data_dir = std::fs::canonicalize("./data").expect("./data must exist");
        let _server = GenericImage::new("ghcr.io/umccr/htsget-rs", "dev-94")
            .with_mapped_port(8080, 8080.tcp())
            .with_mapped_port(8081, 8081.tcp())
            .with_mount(Mount::bind_mount(data_dir.to_string_lossy(), "/data"))
            .start()
            .expect("htsget-rs container should start");
        // htsget-rs prints structured logs but startup is ~150ms; give the
        // workers a moment to bind ports before issuing the first request.
        std::thread::sleep(std::time::Duration::from_secs(1));

        let data = fetch_bytes(
            "http://localhost:8080/reads",
            "data/cram/htsnexus_test_NA12878",
            htsget::reads::Format::Cram,
            Some("11:4900000-5000000"),
        )
        .expect("fetch_bytes succeeds");

        assert!(!data.is_empty(), "htsget response should contain bytes");

        // CRAM parsing may fail without a reference sequence repository — that's
        // a known gap (see follow-up: add `reference` parameter). For now we
        // only assert the fetch path works; uncomment once ref handling lands.
        // let records = parse_cram_records(&data).expect("CRAM decodes");
        // assert!(!records.is_empty());
    }
}
