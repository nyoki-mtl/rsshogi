use crate::{PyBoard, PyMove};
use ::rsshogi::book::{
    BookControl, SbkBook, SbkDiagnostics, SbkEntry, SbkEval, SbkMove, SbkOpenProgress,
    YaneuraOuBook, YaneuraOuBookDiagnostics, YaneuraOuBookEntry, YaneuraOuBookMove,
};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyType};

#[pyclass(from_py_object, name = "YaneuraOuBookDiagnostics")]
#[derive(Clone)]
pub(crate) struct PyYaneuraOuBookDiagnostics {
    inner: YaneuraOuBookDiagnostics,
}

#[pymethods]
impl PyYaneuraOuBookDiagnostics {
    #[getter]
    fn kind(&self) -> &'static str {
        match self.inner {
            YaneuraOuBookDiagnostics::Sorted { .. } => "sorted",
            YaneuraOuBookDiagnostics::Unsorted { .. } => "unsorted",
            YaneuraOuBookDiagnostics::InvalidHeader => "invalid_header",
            YaneuraOuBookDiagnostics::Unvalidated => "unvalidated",
        }
    }

    #[getter]
    fn checked_rows(&self) -> Option<usize> {
        match self.inner {
            YaneuraOuBookDiagnostics::Sorted { checked_rows, .. }
            | YaneuraOuBookDiagnostics::Unsorted { checked_rows, .. } => Some(checked_rows),
            YaneuraOuBookDiagnostics::InvalidHeader | YaneuraOuBookDiagnostics::Unvalidated => None,
        }
    }

    #[getter]
    fn complete(&self) -> Option<bool> {
        match self.inner {
            YaneuraOuBookDiagnostics::Sorted { complete, .. } => Some(complete),
            _ => None,
        }
    }

    #[getter]
    fn line_number(&self) -> Option<usize> {
        match self.inner {
            YaneuraOuBookDiagnostics::Unsorted { line_number, .. } => Some(line_number),
            _ => None,
        }
    }

    #[getter]
    fn previous_sfen(&self) -> Option<String> {
        match &self.inner {
            YaneuraOuBookDiagnostics::Unsorted { previous_sfen, .. } => Some(previous_sfen.clone()),
            _ => None,
        }
    }

    #[getter]
    fn current_sfen(&self) -> Option<String> {
        match &self.inner {
            YaneuraOuBookDiagnostics::Unsorted { current_sfen, .. } => Some(current_sfen.clone()),
            _ => None,
        }
    }

    fn __repr__(&self) -> String {
        format!("YaneuraOuBookDiagnostics(kind={:?})", self.kind())
    }
}

#[pyclass(from_py_object, name = "YaneuraOuBookMove")]
#[derive(Clone)]
pub(crate) struct PyYaneuraOuBookMove {
    inner: YaneuraOuBookMove,
}

#[pymethods]
impl PyYaneuraOuBookMove {
    #[getter]
    fn mv(&self) -> PyMove {
        PyMove::from_move(self.inner.mv())
    }

    #[getter]
    fn ponder(&self) -> PyMove {
        PyMove::from_move(self.inner.ponder())
    }

    #[getter]
    fn score(&self) -> Option<i32> {
        self.inner.score()
    }

    #[getter]
    fn depth(&self) -> Option<u32> {
        self.inner.depth()
    }

    #[getter]
    fn count(&self) -> Option<u64> {
        self.inner.count()
    }

    #[getter]
    fn comment(&self) -> String {
        self.inner.comment().to_string()
    }

    fn __repr__(&self) -> String {
        format!(
            "YaneuraOuBookMove(move={}, ponder={}, score={:?}, depth={:?}, count={:?})",
            self.inner.mv().to_usi(),
            self.inner.ponder().to_usi(),
            self.inner.score(),
            self.inner.depth(),
            self.inner.count()
        )
    }
}

impl PyYaneuraOuBookMove {
    fn from_external_move(inner: YaneuraOuBookMove) -> Self {
        Self { inner }
    }
}

#[pyclass(from_py_object, name = "YaneuraOuBookEntry")]
#[derive(Clone)]
pub(crate) struct PyYaneuraOuBookEntry {
    inner: YaneuraOuBookEntry,
}

#[pymethods]
impl PyYaneuraOuBookEntry {
    #[getter]
    fn sfen(&self) -> &str {
        self.inner.sfen()
    }

    #[getter]
    fn min_ply(&self) -> u32 {
        self.inner.min_ply()
    }

    #[getter]
    fn comment(&self) -> &str {
        self.inner.comment()
    }

    #[getter]
    fn moves(&self) -> Vec<PyYaneuraOuBookMove> {
        self.inner.moves().iter().cloned().map(PyYaneuraOuBookMove::from_external_move).collect()
    }

    fn __len__(&self) -> usize {
        self.inner.moves().len()
    }

    fn __repr__(&self) -> String {
        format!(
            "YaneuraOuBookEntry(sfen={:?}, min_ply={}, moves={})",
            self.inner.sfen(),
            self.inner.min_ply(),
            self.inner.moves().len()
        )
    }
}

impl PyYaneuraOuBookEntry {
    fn from_external_entry(inner: YaneuraOuBookEntry) -> Self {
        Self { inner }
    }
}

#[pyclass(from_py_object, name = "YaneuraOuBook")]
#[derive(Clone)]
pub(crate) struct PyYaneuraOuBook {
    inner: YaneuraOuBook,
}

#[pymethods]
impl PyYaneuraOuBook {
    #[classmethod]
    fn open(_cls: &Bound<'_, PyType>, path: &str) -> PyResult<Self> {
        let inner = YaneuraOuBook::open(path)
            .map_err(|err| PyValueError::new_err(format!("book error: {err}")))?;
        Ok(Self { inner })
    }

    fn diagnostics(&self) -> PyYaneuraOuBookDiagnostics {
        PyYaneuraOuBookDiagnostics { inner: self.inner.diagnostics().clone() }
    }

    fn validate_full(&mut self) -> PyResult<PyYaneuraOuBookDiagnostics> {
        let inner = self
            .inner
            .validate_full()
            .map_err(|err| PyValueError::new_err(format!("book error: {err}")))?;
        Ok(PyYaneuraOuBookDiagnostics { inner })
    }

    fn lookup_sfen(&self, sfen: &str) -> PyResult<Option<PyYaneuraOuBookEntry>> {
        self.inner
            .lookup_sfen(sfen)
            .map(|entry| entry.map(PyYaneuraOuBookEntry::from_external_entry))
            .map_err(|err| PyValueError::new_err(format!("book error: {err}")))
    }

    fn lookup_position(&self, board: &PyBoard) -> PyResult<Option<PyYaneuraOuBookEntry>> {
        self.inner
            .lookup_position(&board.position)
            .map(|entry| entry.map(PyYaneuraOuBookEntry::from_external_entry))
            .map_err(|err| PyValueError::new_err(format!("book error: {err}")))
    }

    fn lookup_sfen_by_scan(&self, sfen: &str) -> PyResult<Option<PyYaneuraOuBookEntry>> {
        self.inner
            .lookup_sfen_by_scan(sfen)
            .map(|entry| entry.map(PyYaneuraOuBookEntry::from_external_entry))
            .map_err(|err| PyValueError::new_err(format!("book error: {err}")))
    }

    fn __repr__(&self) -> String {
        format!("YaneuraOuBook(ordering={})", self.diagnostics().kind())
    }
}

#[pyclass(from_py_object, name = "SbkDiagnostics")]
#[derive(Clone)]
pub(crate) struct PySbkDiagnostics {
    inner: SbkDiagnostics,
}

#[pymethods]
impl PySbkDiagnostics {
    #[getter]
    fn duplicate_positions(&self) -> usize {
        self.inner.duplicate_positions()
    }

    #[getter]
    fn unresolved_next_states(&self) -> usize {
        self.inner.unresolved_next_states()
    }

    fn __repr__(&self) -> String {
        format!(
            "SbkDiagnostics(duplicate_positions={}, unresolved_next_states={})",
            self.inner.duplicate_positions(),
            self.inner.unresolved_next_states()
        )
    }
}

#[pyclass(from_py_object, name = "SbkOpenProgress")]
#[derive(Clone, Copy)]
pub(crate) struct PySbkOpenProgress {
    inner: SbkOpenProgress,
}

#[pymethods]
impl PySbkOpenProgress {
    #[getter]
    fn indexed_states(&self) -> usize {
        self.inner.indexed_states()
    }

    #[getter]
    fn total_states(&self) -> usize {
        self.inner.total_states()
    }

    fn __repr__(&self) -> String {
        format!(
            "SbkOpenProgress(indexed_states={}, total_states={})",
            self.inner.indexed_states(),
            self.inner.total_states()
        )
    }
}

#[pyclass(from_py_object, name = "SbkEval")]
#[derive(Clone)]
pub(crate) struct PySbkEval {
    inner: SbkEval,
}

#[pymethods]
impl PySbkEval {
    #[getter]
    fn evaluation_value(&self) -> Option<i32> {
        self.inner.evaluation_value()
    }

    #[getter]
    fn depth(&self) -> Option<i32> {
        self.inner.depth()
    }

    #[getter]
    fn sel_depth(&self) -> Option<i32> {
        self.inner.sel_depth()
    }

    #[getter]
    fn nodes(&self) -> Option<i64> {
        self.inner.nodes()
    }

    #[getter]
    fn variation(&self) -> Option<&str> {
        self.inner.variation()
    }

    #[getter]
    fn engine_name(&self) -> Option<&str> {
        self.inner.engine_name()
    }
}

impl PySbkEval {
    fn from_sbk_eval(inner: SbkEval) -> Self {
        Self { inner }
    }
}

#[pyclass(from_py_object, name = "SbkMove")]
#[derive(Clone)]
pub(crate) struct PySbkMove {
    inner: SbkMove,
}

#[pymethods]
impl PySbkMove {
    #[getter]
    fn mv(&self) -> PyMove {
        PyMove::from_move(self.inner.mv())
    }

    #[getter]
    fn raw_move(&self) -> i32 {
        self.inner.raw_move()
    }

    #[getter]
    fn evaluation(&self) -> Option<i32> {
        self.inner.evaluation()
    }

    #[getter]
    fn weight(&self) -> Option<i32> {
        self.inner.weight()
    }

    #[getter]
    fn next_state_id(&self) -> Option<i32> {
        self.inner.next_state_id()
    }

    fn __repr__(&self) -> String {
        format!(
            "SbkMove(move={}, raw_move={}, evaluation={:?}, weight={:?}, next_state_id={:?})",
            self.inner.mv().to_usi(),
            self.inner.raw_move(),
            self.inner.evaluation(),
            self.inner.weight(),
            self.inner.next_state_id()
        )
    }
}

impl PySbkMove {
    fn from_sbk_move(inner: SbkMove) -> Self {
        Self { inner }
    }
}

#[pyclass(from_py_object, name = "SbkEntry")]
#[derive(Clone)]
pub(crate) struct PySbkEntry {
    inner: SbkEntry,
}

#[pymethods]
impl PySbkEntry {
    #[getter]
    fn state_id(&self) -> Option<i32> {
        self.inner.state_id()
    }

    #[getter]
    fn state_index(&self) -> usize {
        self.inner.state_index()
    }

    #[getter]
    fn sfen(&self) -> &str {
        self.inner.sfen()
    }

    #[getter]
    fn games(&self) -> Option<i32> {
        self.inner.games()
    }

    #[getter]
    fn won_black(&self) -> Option<i32> {
        self.inner.won_black()
    }

    #[getter]
    fn won_white(&self) -> Option<i32> {
        self.inner.won_white()
    }

    #[getter]
    fn comment(&self) -> Option<&str> {
        self.inner.comment()
    }

    #[getter]
    fn moves(&self) -> Vec<PySbkMove> {
        self.inner.moves().iter().cloned().map(PySbkMove::from_sbk_move).collect()
    }

    #[getter]
    fn evals(&self) -> Vec<PySbkEval> {
        self.inner.evals().iter().cloned().map(PySbkEval::from_sbk_eval).collect()
    }

    fn __len__(&self) -> usize {
        self.inner.moves().len()
    }

    fn __repr__(&self) -> String {
        format!(
            "SbkEntry(state_index={}, state_id={:?}, moves={}, evals={})",
            self.inner.state_index(),
            self.inner.state_id(),
            self.inner.moves().len(),
            self.inner.evals().len()
        )
    }
}

impl PySbkEntry {
    fn from_sbk_entry(inner: SbkEntry) -> Self {
        Self { inner }
    }
}

#[pyclass(from_py_object, name = "SbkBook")]
#[derive(Clone)]
pub(crate) struct PySbkBook {
    inner: SbkBook,
}

#[pymethods]
impl PySbkBook {
    #[classmethod]
    fn open(_cls: &Bound<'_, PyType>, path: &str) -> PyResult<Self> {
        let inner = SbkBook::open(path)
            .map_err(|err| PyValueError::new_err(format!("book error: {err}")))?;
        Ok(Self { inner })
    }

    #[classmethod]
    fn open_with_control(
        _cls: &Bound<'_, PyType>,
        path: &str,
        callback: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let mut callback_error = None;
        let result = SbkBook::open_with_control(path, |progress| {
            if callback_error.is_some() {
                return BookControl::Cancel;
            }
            match callback.call1((PySbkOpenProgress { inner: progress },)) {
                Ok(value) => {
                    if value.is_none() {
                        BookControl::Continue
                    } else {
                        match value.is_truthy() {
                            Ok(true) => BookControl::Continue,
                            Ok(false) => BookControl::Cancel,
                            Err(err) => {
                                callback_error = Some(err);
                                BookControl::Cancel
                            }
                        }
                    }
                }
                Err(err) => {
                    callback_error = Some(err);
                    BookControl::Cancel
                }
            }
        });
        if let Some(err) = callback_error {
            return Err(err);
        }
        let inner = result.map_err(|err| PyValueError::new_err(format!("book error: {err}")))?;
        Ok(Self { inner })
    }

    #[getter]
    fn author(&self) -> Option<&str> {
        self.inner.author()
    }

    #[getter]
    fn description(&self) -> Option<&str> {
        self.inner.description()
    }

    fn diagnostics(&self) -> PySbkDiagnostics {
        PySbkDiagnostics { inner: self.inner.diagnostics().clone() }
    }

    fn lookup_sfen(&self, sfen: &str) -> PyResult<Option<PySbkEntry>> {
        self.inner
            .lookup_sfen(sfen)
            .map(|entry| entry.map(PySbkEntry::from_sbk_entry))
            .map_err(|err| PyValueError::new_err(format!("book error: {err}")))
    }

    fn lookup_position(&self, board: &PyBoard) -> PyResult<Option<PySbkEntry>> {
        self.inner
            .lookup_position(&board.position)
            .map(|entry| entry.map(PySbkEntry::from_sbk_entry))
            .map_err(|err| PyValueError::new_err(format!("book error: {err}")))
    }

    fn lookup_state_id(&self, state_id: i32) -> PyResult<Option<PySbkEntry>> {
        self.inner
            .lookup_state_id(state_id)
            .map(|entry| entry.map(PySbkEntry::from_sbk_entry))
            .map_err(|err| PyValueError::new_err(format!("book error: {err}")))
    }

    fn lookup_state_index(&self, state_index: usize) -> PyResult<Option<PySbkEntry>> {
        self.inner
            .lookup_state_index(state_index)
            .map(|entry| entry.map(PySbkEntry::from_sbk_entry))
            .map_err(|err| PyValueError::new_err(format!("book error: {err}")))
    }

    fn child_entry(
        &self,
        parent: PyRef<'_, PySbkEntry>,
        move_index: usize,
    ) -> PyResult<Option<PySbkEntry>> {
        self.inner
            .child_entry(&parent.inner, move_index)
            .map(|entry| entry.map(PySbkEntry::from_sbk_entry))
            .map_err(|err| PyValueError::new_err(format!("book error: {err}")))
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __repr__(&self) -> String {
        format!("SbkBook(len={})", self.inner.len())
    }
}

pub(crate) fn register_book_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyYaneuraOuBookDiagnostics>()?;
    m.add_class::<PyYaneuraOuBookMove>()?;
    m.add_class::<PyYaneuraOuBookEntry>()?;
    m.add_class::<PyYaneuraOuBook>()?;
    m.add_class::<PySbkDiagnostics>()?;
    m.add_class::<PySbkOpenProgress>()?;
    m.add_class::<PySbkEval>()?;
    m.add_class::<PySbkMove>()?;
    m.add_class::<PySbkEntry>()?;
    m.add_class::<PySbkBook>()?;
    Ok(())
}
