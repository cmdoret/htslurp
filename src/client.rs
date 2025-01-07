use pyo3::prelude::*;
use io::Error;
use futures::TryStreamExt;
use tokio::io::{self, AsyncWriteExt};
use noodles::htsget as htsget;

// e.g. stream("http://localhost/htsget", "dir/file", "chr1:123-4541")
#[pyfunction]
pub fn stream(base_url: &str, id: &str, region: &str) -> Result<(), Error>{
    let format = htsget::reads::Format::Cram;
    let url = base_url.parse().unwrap();
    let client = htsget::Client::new(url);
    // TODO: generic on variants and reads
    let mut request = client.reads(id);
    request = request.set_format(format);

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            request = request.add_region(region.parse().expect("invalid region"));
            let response = request.send().await.expect("request failed");
            let mut chunks = response.chunks();
            let mut stdout = io::stdout();

            while let Some(chunk) = chunks.try_next().await.expect("can't load chunk") {
                stdout.write_all(&chunk).await.expect("can't write");
            }
        });


    Ok(())
}

#[pymodule]
pub fn client(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(stream, m)?)?;
    Ok(())
}

/// Test client streaming. Assumes an htsget server is already running on localhost

#[cfg(test)]
pub mod tests {
    use super::*;
    use testcontainers::{
        core::{
            IntoContainerPort,
            Mount,
            logs::LogFrame,
        },
        runners::SyncRunner,
        GenericImage, ImageExt,
    };


    #[test]
    fn test_stream() -> Result<(), Error>{
        let _server = GenericImage::new("ghcr.io/umccr/htsget-rs", "dev-94")
          .with_mapped_port(8080, 8080.tcp())
          .with_mapped_port(8081, 8081.tcp())
          .with_mount(
            Mount::bind_mount(
                std::fs::canonicalize("./data")?
                    .to_string_lossy(),
                "/data"
            )
          )
        .with_log_consumer(move |frame: &LogFrame| {
            println!("{}", String::from_utf8_lossy(frame.bytes()));
        })
          .start()
          .unwrap();

        std::thread::sleep(std::time::Duration::from_secs(3));

        let base_url = "http://localhost:8080/reads";
        let id = "data/cram/htsnexus_test_NA12878";
        let region = "11:4900000-5000000";
        stream(base_url, id, region).unwrap();
        Ok(())
    }
}
