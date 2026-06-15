use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::wrap_pymodule;
mod client;
mod parse;
mod stream;

/// Lazy iterator over alignment records returned by ``stream_records``.
///
/// Each item is a ``bytes`` object holding one SAM-format alignment line,
/// decoded on demand by a background worker thread (so memory stays bounded).
/// The ``header`` attribute is the SAM header as ``bytes``, e.g. to rebuild a
/// ``pysam.AlignmentHeader``.
#[pyclass]
pub struct RecordIter {
    pub(crate) header: Vec<u8>,
    pub(crate) rx: stream::RecordRx,
}

#[pymethods]
impl RecordIter {
    /// SAM header text (as ``bytes``) for the records yielded by this iterator.
    #[getter]
    fn header<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.header)
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>) -> PyResult<Option<Py<PyAny>>> {
        let py = slf.py();
        // Pull rx out of the PyRefMut so the closure captures only a `Send`
        // reference, not the whole non-Send PyRefMut.
        let rx = &mut slf.rx;
        let item = py.detach(move || rx.blocking_recv());
        match item {
            Some(Ok(bytes)) => Ok(Some(PyBytes::new(py, &bytes).unbind().into())),
            Some(Err(e)) => Err(PyRuntimeError::new_err(e.to_string())),
            None => Ok(None),
        }
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
