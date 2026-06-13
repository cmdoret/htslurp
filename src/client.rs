use crate::stream::start_stream;
use crate::RecordIter;
use noodles::{core::Region, htsget};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::path::PathBuf;

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
    py: Python<'_>,
    base_url: &str,
    id: &str,
    format: &str,
    region: Option<&str>,
    reference: Option<&str>,
) -> PyResult<RecordIter> {
    let fmt = match format {
        "CRAM" => htsget::reads::Format::Cram,
        "BAM" => htsget::reads::Format::Bam,
        other => {
            return Err(PyRuntimeError::new_err(format!(
                "unsupported format: {other}"
            )))
        }
    };
    let parsed_region: Option<Region> = region
        .map(|s| s.parse::<Region>())
        .transpose()
        .map_err(|e| PyRuntimeError::new_err(format!("invalid region: {e}")))?;
    let base_url = base_url.to_string();
    let id = id.to_string();
    let reference = reference.map(PathBuf::from);
    // start_stream blocks until the worker has fetched and decoded the header
    // (a full network round-trip). Release the GIL so other Python threads
    // aren't frozen for the duration.
    let (header, rx) = py
        .detach(move || start_stream(base_url, id, fmt, parsed_region, reference))
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

    fn assert_region_records(label: &str, header: &[u8], rx: &mut crate::stream::RecordRx) {
        assert!(
            !header.is_empty(),
            "{label}: SAM header should be non-empty"
        );
        assert!(
            header.starts_with(b"@HD") || header.starts_with(b"@SQ"),
            "{label}: SAM header should start with @HD or @SQ"
        );

        let mut records = Vec::new();
        while let Some(item) = rx.blocking_recv() {
            records.push(item.unwrap_or_else(|e| panic!("{label}: record error: {e}")));
        }
        assert!(!records.is_empty(), "{label}: stream should yield records");

        for bytes in &records {
            let line = std::str::from_utf8(bytes).expect("SAM line is UTF-8");
            let fields: Vec<&str> = line.split('\t').collect();
            assert!(
                fields.len() >= 11,
                "{label}: SAM line should have 11+ fields, got {}: {line:?}",
                fields.len()
            );
            assert_eq!(
                fields[2], "11",
                "{label}: RNAME should be 11, got {}",
                fields[2]
            );
            let pos: u32 = fields[3].parse().expect("POS is integer");
            assert!(
                pos < 5_000_000,
                "{label}: POS {pos} should start before region end (overlap, not after)"
            );
        }
        eprintln!("[{label}] total records: {}", records.len());
        eprintln!(
            "[{label}] first record: {}",
            String::from_utf8_lossy(&records[0])
        );
    }

    // Requires Docker. Run with `cargo test -- --ignored`.
    // Exercises both BAM and CRAM streaming paths against the umccr htsget-rs
    // reference server. Kept as a single test so the two formats can share one
    // container (testcontainers pins host ports 8080/8081, so parallel tests
    // would conflict).
    #[test]
    #[ignore]
    fn streaming_against_htsget_rs() {
        let data_dir = std::fs::canonicalize("./data").expect("./data must exist");
        let _server = GenericImage::new("ghcr.io/umccr/htsget-rs", "dev-94")
            .with_mapped_port(8080, 8080.tcp())
            .with_mapped_port(8081, 8081.tcp())
            .with_mount(Mount::bind_mount(data_dir.to_string_lossy(), "/data"))
            .start()
            .expect("htsget-rs container should start");
        std::thread::sleep(std::time::Duration::from_secs(1));

        let region: Region = "11:4900000-5000000".parse().expect("region parses");

        // CRAM path
        let (header, mut rx) = start_stream(
            "http://localhost:8080/reads".to_string(),
            "data/cram/htsnexus_test_NA12878".to_string(),
            htsget::reads::Format::Cram,
            Some(region.clone()),
            None,
        )
        .expect("CRAM stream starts");
        assert_region_records("CRAM", &header, &mut rx);

        // BAM path (same dataset, converted upstream via samtools)
        let (header, mut rx) = start_stream(
            "http://localhost:8080/reads".to_string(),
            "data/bam/htsnexus_test_NA12878".to_string(),
            htsget::reads::Format::Bam,
            Some(region),
            None,
        )
        .expect("BAM stream starts");
        assert_region_records("BAM", &header, &mut rx);
    }
}
