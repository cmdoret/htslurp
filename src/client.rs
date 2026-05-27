use std::path::Path;
use pyo3::prelude::*;
use pyo3::exceptions::PyRuntimeError;
use futures::TryStreamExt;
use noodles::{core::Region, htsget};
use crate::parse::{parse_bam_records, parse_cram_records};
use crate::RecordIter;

fn io_err(kind: std::io::ErrorKind, e: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(kind, e.to_string())
}

fn fetch_bytes(
    base_url: &str,
    id: &str,
    format: htsget::reads::Format,
    region: Option<&Region>,
) -> Result<Vec<u8>, std::io::Error> {
    let url = base_url
        .parse()
        .map_err(|e| io_err(std::io::ErrorKind::InvalidInput, e))?;
    let client = htsget::Client::new(url);
    let mut request = client.reads(id).set_format(format);
    if let Some(r) = region {
        request = request.add_region(r.clone());
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
#[pyo3(signature = (base_url, id, format, region=None, reference=None))]
pub fn stream_records(
    base_url: &str,
    id: &str,
    format: &str,
    region: Option<&str>,
    reference: Option<&str>,
) -> PyResult<RecordIter> {
    let fmt = match format {
        "CRAM" => htsget::reads::Format::Cram,
        "BAM" => htsget::reads::Format::Bam,
        other => return Err(PyRuntimeError::new_err(format!("unsupported format: {other}"))),
    };
    let parsed_region: Option<Region> = region
        .map(|s| s.parse::<Region>())
        .transpose()
        .map_err(|e| PyRuntimeError::new_err(format!("invalid region: {e}")))?;
    let data = fetch_bytes(base_url, id, fmt, parsed_region.as_ref())
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    let records = match fmt {
        htsget::reads::Format::Cram => {
            parse_cram_records(&data, reference.map(Path::new), parsed_region.as_ref())
        }
        htsget::reads::Format::Bam => parse_bam_records(&data, parsed_region.as_ref()),
    }
    .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
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

        let region: Region = "11:4900000-5000000".parse().expect("region parses");
        let data = fetch_bytes(
            "http://localhost:8080/reads",
            "data/cram/htsnexus_test_NA12878",
            htsget::reads::Format::Cram,
            Some(&region),
        )
        .expect("fetch_bytes succeeds");

        assert!(!data.is_empty(), "htsget response should contain bytes");

        let records = parse_cram_records(&data, None, Some(&region)).expect("CRAM decodes");
        assert!(!records.is_empty(), "CRAM response should yield records");

        // Every returned record must be on chr 11 and overlap the requested region.
        for bytes in &records {
            let line = std::str::from_utf8(bytes).expect("SAM line is UTF-8");
            let fields: Vec<&str> = line.split('\t').collect();
            assert_eq!(fields[2], "11", "RNAME should be 11, got {}", fields[2]);
            let pos: u32 = fields[3].parse().expect("POS is integer");
            assert!(
                pos < 5_000_000,
                "POS {pos} should start before region end (overlap, not after)"
            );
        }
        eprintln!("first record (SAM): {}", String::from_utf8_lossy(&records[0]));
        eprintln!("total records: {}", records.len());

        // htsget hands back full BGZF blocks that overhang the requested
        // region, so the unfiltered count should be strictly greater. If this
        // ever inverts, htsget got more precise or the fixture changed.
        let unfiltered = parse_cram_records(&data, None, None).expect("CRAM decodes");
        eprintln!("unfiltered records: {}", unfiltered.len());
        assert!(
            unfiltered.len() > records.len(),
            "filter should drop flanking records (unfiltered={}, filtered={})",
            unfiltered.len(),
            records.len(),
        );
    }
}
