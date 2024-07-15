use pyo3::prelude::*;
use io::Error;
use futures::TryStreamExt;
use tokio::io::{self, AsyncWriteExt};
use noodles_htsget as htsget;

#[pyfunction]
pub async fn stream(base_url: &str, id: &str, region: &str) -> Result<(), Error>{
    let url = base_url.parse().unwrap();
    let client = htsget::Client::new(url);
    // TODO: generic on variants and reads
    let mut request = client.reads(id);

    request = request.add_region(region.parse().expect("invalid region"));

    let response = request.send().await.expect("request failed");
    let mut chunks = response.chunks();
    let mut stdout = io::stdout();

    while let Some(chunk) = chunks.try_next().await.expect("can't load chunk") {
        stdout.write_all(&chunk).await?;
    }

    Ok(())
}

/// A Python module implemented in Rust.
#[pymodule]
fn client(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(stream, m)?)?;
    Ok(())
}
