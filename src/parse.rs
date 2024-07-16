use pyo3::prelude::*;
use std::io::Error;
use noodles::cram::io::reader::Reader;

// Returns an iterator over CRAM/BAM records created from incoming bytes.
#[pyfunction]
pub fn parse_reads(data: &[u8], format: &str) -> Result<(), Error> {
    match format {
        "CRAM" => Ok(()),
        "BAM" => Ok(()),
        _ => panic!("not implemented yet")
    }
}

pub fn parse_variants(data: &[u8], format: &str) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        "VCF" => Ok(()),
        "BCF" => Ok(()),
        _ => panic!("not implemented yet")
    }
}
