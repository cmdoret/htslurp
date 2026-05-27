use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::wrap_pymodule;
mod client;
mod parse;

/// Iterator over alignment records returned by ``stream_records``.
///
/// Each item is a ``bytes`` object containing one SAM-format alignment line
/// (tab-separated, no trailing newline guarantees). The ``header`` attribute
/// returns the full SAM header as ``bytes`` so callers can reconstruct a
/// ``pysam.AlignmentHeader`` and parse records with ``AlignedSegment.fromstring``.
#[pyclass]
pub struct RecordIter {
    pub(crate) header: Vec<u8>,
    pub(crate) records: Vec<Vec<u8>>,
    pub(crate) index: usize,
}

#[pymethods]
impl RecordIter {
    /// SAM header text (as ``bytes``) for the records yielded by this iterator.
    #[getter]
    fn header<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new_bound(py, &self.header)
    }

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
