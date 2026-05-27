use std::path::Path;
use noodles::{bam, cram, fasta, sam};
use noodles::sam::alignment::io::Write as AlignmentWrite;

fn record_to_sam_bytes(
    header: &sam::Header,
    record: &impl sam::alignment::Record,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut buf = Vec::new();
    let mut writer = sam::io::Writer::new(&mut buf);
    writer.write_alignment_record(header, record)?;
    Ok(buf)
}

pub fn parse_bam_records(data: &[u8]) -> Result<Vec<Vec<u8>>, Box<dyn std::error::Error>> {
    let mut reader = bam::io::Reader::new(std::io::Cursor::new(data));
    let header = reader.read_header()?;
    let mut out = Vec::new();
    for result in reader.records() {
        out.push(record_to_sam_bytes(&header, &result?)?);
    }
    Ok(out)
}

pub fn parse_cram_records(
    data: &[u8],
    reference: Option<&Path>,
) -> Result<Vec<Vec<u8>>, Box<dyn std::error::Error>> {
    let repo = match reference {
        Some(path) => fasta::io::indexed_reader::Builder::default()
            .build_from_path(path)
            .map(fasta::repository::adapters::IndexedReader::new)
            .map(fasta::Repository::new)?,
        None => fasta::Repository::default(),
    };
    let mut reader = cram::io::reader::Builder::default()
        .set_reference_sequence_repository(repo)
        .build_from_reader(std::io::Cursor::new(data));
    reader.read_file_definition()?;
    let header = reader.read_file_header()?;
    let mut out = Vec::new();
    for result in reader.records(&header) {
        out.push(record_to_sam_bytes(&header, &result?)?);
    }
    Ok(out)
}
