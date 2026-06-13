use noodles::core::region::Interval;
use noodles::sam::alignment::io::Write as AlignmentWrite;
use noodles::{core::Region, sam};

pub(crate) fn record_to_sam_bytes(
    header: &sam::Header,
    record: &impl sam::alignment::Record,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut buf = Vec::new();
    let mut writer = sam::io::Writer::new(&mut buf);
    writer.write_alignment_record(header, record)?;
    // noodles terminates each record with '\n'; drop it so callers get one
    // clean SAM line (a trailing newline dirties split('\t')[-1] and is
    // rejected by pysam.AlignedSegment.fromstring).
    if buf.last() == Some(&b'\n') {
        buf.pop();
    }
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
// extra flanking records), so each record is re-checked against the request.
//
// The reference-sequence index and interval are invariant across the stream,
// so they are resolved once here rather than per record.
pub(crate) struct RegionFilter {
    // `None` when the region's reference name is absent from the header, in
    // which case no record can match.
    target_ref_id: Option<usize>,
    interval: Interval,
}

impl RegionFilter {
    pub(crate) fn new(header: &sam::Header, region: &Region) -> Self {
        Self {
            target_ref_id: header.reference_sequences().get_index_of(region.name()),
            interval: region.interval(),
        }
    }

    pub(crate) fn matches<R: sam::alignment::Record>(
        &self,
        record: &R,
        header: &sam::Header,
    ) -> bool {
        let Some(target_ref_id) = self.target_ref_id else {
            return false;
        };
        let Some(Ok(ref_id)) = record.reference_sequence_id(header) else {
            return false;
        };
        if ref_id != target_ref_id {
            return false;
        }
        // Records without a resolvable start/end (e.g. placed-but-unmapped
        // reads) are dropped rather than included: we can't position them
        // against the interval. This is a deliberate policy, not a missed edge
        // case.
        let (Some(Ok(start)), Some(Ok(end))) = (record.alignment_start(), record.alignment_end())
        else {
            return false;
        };
        self.interval.intersects((start..=end).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use noodles::sam::alignment::RecordBuf;

    #[test]
    fn record_to_sam_bytes_omits_trailing_newline() {
        let header = sam::Header::default();
        let record = RecordBuf::default();
        let bytes = record_to_sam_bytes(&header, &record).unwrap();
        assert!(!bytes.is_empty(), "record bytes should be non-empty");
        assert!(
            !bytes.ends_with(b"\n"),
            "record bytes must not end with a newline (it would dirty split('\\t')[-1] \
             and break pysam.AlignedSegment.fromstring); got {:?}",
            String::from_utf8_lossy(&bytes)
        );
    }
}
