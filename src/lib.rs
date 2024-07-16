use pyo3::prelude::*;
use pyo3::wrap_pymodule;
mod client;
mod parse;

#[pymodule]
mod parser {
    use crate::parse;

    #[pymodule_export]
    use parse::parse_reads;
}
#[pymodule]
mod htsget_client {
    use crate::client;

    #[pymodule_export]
    use client::stream;
}

#[pymodule]
fn htslurp(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_wrapped(wrap_pymodule!(htsget_client))?;

    Ok(())
}
