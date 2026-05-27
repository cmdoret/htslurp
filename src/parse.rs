use std::path::Path;
use noodles::{bam, core::Region, cram, fasta, sam};
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

// htsget responses are coarser than the requested region (BGZF blocks span
// extra flanking records), so callers receive records the user did not ask
// for unless we re-check each one here.
fn matches_region<R: sam::alignment::Record>(
    record: &R,
    header: &sam::Header,
    region: &Region,
) -> bool {
    let Some(target_ref_id) = header.reference_sequences().get_index_of(region.name()) else {
        return false;
    };
    let Some(Ok(ref_id)) = record.reference_sequence_id(header) else {
        return false;
    };
    if ref_id != target_ref_id {
        return false;
    }
    let (Some(Ok(start)), Some(Ok(end))) =
        (record.alignment_start(), record.alignment_end())
    else {
        return false;
    };
    region.interval().intersects((start..=end).into())
}

pub fn parse_bam_records(
    data: &[u8],
    region: Option<&Region>,
) -> Result<Vec<Vec<u8>>, Box<dyn std::error::Error>> {
    let mut reader = bam::io::Reader::new(std::io::Cursor::new(data));
    let header = reader.read_header()?;
    let mut out = Vec::new();
    for result in reader.records() {
        let record = result?;
        if let Some(r) = region {
            if !matches_region(&record, &header, r) {
                continue;
            }
        }
        out.push(record_to_sam_bytes(&header, &record)?);
    }
    Ok(out)
}

pub fn parse_cram_records(
    data: &[u8],
    reference: Option<&Path>,
    region: Option<&Region>,
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
        let record = result?;
        if let Some(r) = region {
            if !matches_region(&record, &header, r) {
                continue;
            }
        }
        out.push(record_to_sam_bytes(&header, &record)?);
    }
    Ok(out)
}
