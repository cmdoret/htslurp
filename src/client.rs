use std::path::PathBuf;
use pyo3::prelude::*;
use pyo3::exceptions::PyRuntimeError;
use noodles::{core::Region, htsget};
use crate::stream::start_stream;
use crate::RecordIter;

/// Stream alignment records from an htsget server.
///
/// Lazily fetches and decodes a (possibly region-restricted) slice of a BAM
/// or CRAM resource. Memory usage is bounded by an internal record buffer;
/// the full response is never materialized.
///
/// Args:
///     base_url: htsget endpoint URL (e.g. ``"https://htsget.ga4gh.org/reads"``).
///     id: Resource identifier on the server (e.g. ``"giab.NA12878"``).
///     format: ``"BAM"`` or ``"CRAM"``.
///     region: Optional genomic region string (e.g. ``"chr1:1000-2000"``).
///         When set, records that don't overlap are dropped.
///     reference: Optional path to an indexed FASTA. Required only for CRAMs
///         that use external reference-based compression.
///
/// Returns:
///     A ``RecordIter`` yielding ``bytes`` (SAM lines). ``RecordIter.header``
///     is the SAM header as ``bytes``.
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
    let (header, rx) = start_stream(
        base_url.to_string(),
        id.to_string(),
        fmt,
        parsed_region,
        reference.map(PathBuf::from),
    )
    .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    Ok(RecordIter { header, rx })
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
        std::thread::sleep(std::time::Duration::from_secs(1));

        let region: Region = "11:4900000-5000000".parse().expect("region parses");
        let (header, mut rx) = start_stream(
            "http://localhost:8080/reads".to_string(),
            "data/cram/htsnexus_test_NA12878".to_string(),
            htsget::reads::Format::Cram,
            Some(region),
            None,
        )
        .expect("stream starts");

        assert!(!header.is_empty(), "SAM header should be non-empty");
        assert!(
            header.starts_with(b"@HD") || header.starts_with(b"@SQ"),
            "SAM header should start with @HD or @SQ"
        );

        let mut records = Vec::new();
        while let Some(item) = rx.blocking_recv() {
            records.push(item.expect("record decodes"));
        }
        assert!(!records.is_empty(), "stream should yield records");

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
    }
}
