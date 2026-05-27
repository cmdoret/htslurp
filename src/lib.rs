use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::wrap_pymodule;
mod client;
mod parse;

#[pyclass]
pub struct RecordIter {
    pub(crate) records: Vec<Vec<u8>>,
    pub(crate) index: usize,
}

#[pymethods]
impl RecordIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>) -> Option<PyObject> {
        if slf.index >= slf.records.len() {
            return None;
        }
        let idx = slf.index;
        slf.index += 1;
        let bytes = std::mem::take(&mut slf.records[idx]);
        let py = slf.py();
        Some(PyBytes::new_bound(py, &bytes).unbind().into())
    }
}

#[pymodule]
mod htsget_client {
    #[pymodule_export]
    use crate::client::stream_records;
    #[pymodule_export]
    use crate::RecordIter;
}

#[pymodule]
fn htslurp(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_wrapped(wrap_pymodule!(htsget_client))?;
    m.add_class::<RecordIter>()?;
    Ok(())
}
