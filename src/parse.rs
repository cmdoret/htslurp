use noodles::{bam, cram, sam};

pub fn parse_bam_records(data: &[u8]) -> Result<Vec<Vec<u8>>, Box<dyn std::error::Error>> {
    let mut reader = bam::io::Reader::new(std::io::Cursor::new(data));
    let header = reader.read_header()?;
    let mut out = Vec::new();
    for result in reader.records() {
        let record = result?;
        let mut buf = Vec::new();
        let mut writer = sam::io::Writer::new(&mut buf);
        writer.write_alignment_record(&header, &record)?;
        out.push(buf);
    }
    Ok(out)
}

pub fn parse_cram_records(data: &[u8]) -> Result<Vec<Vec<u8>>, Box<dyn std::error::Error>> {
    let mut reader = cram::io::Reader::new(std::io::Cursor::new(data));
    reader.read_file_definition()?;
    let header = reader.read_file_header()?;
    let mut out = Vec::new();
    for result in reader.records(&header) {
        let record = result?;
        let mut buf = Vec::new();
        let mut writer = sam::io::Writer::new(&mut buf);
        writer.write_alignment_record(&header, &record)?;
        out.push(buf);
    }
    Ok(out)
}
