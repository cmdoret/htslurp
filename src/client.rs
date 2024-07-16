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
