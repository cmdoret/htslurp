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
// extra flanking records), so each record is re-checked. The reference index
// and interval are invariant per stream, so they're resolved once.
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
        // Drop records without a resolvable start/end (e.g. placed-but-unmapped
        // reads): they can't be positioned against the interval.
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
    use noodles::core::Position;
    use noodles::sam::alignment::record::cigar::{op::Kind, Op};
    use noodles::sam::alignment::record_buf::Cigar;
    use noodles::sam::alignment::RecordBuf;
    use noodles::sam::header::record::value::{map::ReferenceSequence, Map};
    use proptest::prelude::*;
    use std::num::NonZeroUsize;

    // A header with two references, "11" at index 0 and "12" at index 1.
    fn header_with_refs() -> sam::Header {
        let len = NonZeroUsize::new(1_000_000).unwrap();
        sam::Header::builder()
            .add_reference_sequence("11", Map::<ReferenceSequence>::new(len))
            .add_reference_sequence("12", Map::<ReferenceSequence>::new(len))
            .build()
    }

    // A mapped record on `reference_sequence_id` spanning [start, start + span - 1]
    // (a single `{span}M` cigar yields that alignment end).
    fn mapped_record(reference_sequence_id: usize, start: usize, span: usize) -> RecordBuf {
        RecordBuf::builder()
            .set_reference_sequence_id(reference_sequence_id)
            .set_alignment_start(Position::new(start).unwrap())
            .set_cigar(Cigar::from(vec![Op::new(Kind::Match, span)]))
            .build()
    }

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

    #[test]
    fn region_filter_drops_unmapped_record() {
        let header = header_with_refs();
        let region: Region = "11:1-1000000".parse().unwrap();
        let filter = RegionFilter::new(&header, &region);
        // Default record has no reference id and no alignment start.
        assert!(!filter.matches(&RecordBuf::default(), &header));
    }

    proptest! {
        // No on-disk regression seeds (keeps the repo root tidy).
        #![proptest_config(ProptestConfig { failure_persistence: None, ..ProptestConfig::default() })]

        // On the requested reference, a record matches exactly when its
        // [start, end] overlaps the requested interval (both closed).
        #[test]
        fn region_filter_matches_iff_intervals_overlap(
            rec_start in 1usize..2000,
            rec_span in 1usize..300,
            reg_start in 1usize..2000,
            reg_span in 1usize..300,
        ) {
            let header = header_with_refs();
            let reg_end = reg_start + reg_span - 1;
            let region: Region = format!("11:{reg_start}-{reg_end}").parse().unwrap();
            let filter = RegionFilter::new(&header, &region);

            let record = mapped_record(0, rec_start, rec_span);
            let rec_end = rec_start + rec_span - 1;
            let overlaps = rec_start <= reg_end && reg_start <= rec_end;

            prop_assert_eq!(filter.matches(&record, &header), overlaps);
        }

        // A record on a different reference never matches, whatever its position.
        #[test]
        fn region_filter_rejects_other_reference(
            rec_start in 1usize..2000,
            rec_span in 1usize..300,
        ) {
            let header = header_with_refs();
            let region: Region = "11:1-1000000".parse().unwrap();
            let filter = RegionFilter::new(&header, &region);
            let record = mapped_record(1, rec_start, rec_span); // "12"
            prop_assert!(!filter.matches(&record, &header));
        }

        // If the requested reference name is absent from the header, nothing matches.
        #[test]
        fn region_filter_rejects_when_name_absent_from_header(
            rec_start in 1usize..2000,
            rec_span in 1usize..300,
        ) {
            let header = header_with_refs();
            let region: Region = "ZZ:1-1000000".parse().unwrap();
            let filter = RegionFilter::new(&header, &region);
            let record = mapped_record(0, rec_start, rec_span);
            prop_assert!(!filter.matches(&record, &header));
        }
    }
}
