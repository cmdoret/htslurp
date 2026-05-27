use noodles::{core::Region, sam};
use noodles::sam::alignment::io::Write as AlignmentWrite;

pub(crate) fn record_to_sam_bytes(
    header: &sam::Header,
    record: &impl sam::alignment::Record,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut buf = Vec::new();
    let mut writer = sam::io::Writer::new(&mut buf);
    writer.write_alignment_record(header, record)?;
    Ok(buf)
}

pub(crate) fn header_to_sam_bytes(
    header: &sam::Header,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut buf = Vec::new();
    let mut writer = sam::io::Writer::new(&mut buf);
    writer.write_header(header)?;
    Ok(buf)
}

// htsget responses are coarser than the requested region (BGZF blocks span
// extra flanking records), so we re-check each one here.
pub(crate) fn matches_region<R: sam::alignment::Record>(
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
