use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::{PyBoard, PyMove};
use ::rsshogi::book::{
    BookError, NativeBook, NativeBookBuilder, NativeBookEditor, NativeBookEntry,
    NativeBookMetadata, NativeBookPosition, NativeBookSalvageReport, NativeBound, NativeCandidate,
    NativeScore,
};
use pyo3::exceptions::{PyBlockingIOError, PyOverflowError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;

pyo3::create_exception!(_rsshogi, NativeBookLogError, PyValueError);
pyo3::create_exception!(_rsshogi, NativeBookSaveUncertainError, PyRuntimeError);

fn error(error: BookError) -> PyErr {
    match error {
        BookError::Io(error) => error.into(),
        BookError::Busy => PyBlockingIOError::new_err(error.to_string()),
        BookError::NativeLog { offset, valid_end, sequence, reason } => Python::attach(|py| {
            let exception = NativeBookLogError::new_err(format!(
                "invalid native book log at {offset}: {reason}"
            ));
            let value = exception.value(py);
            for (name, number) in
                [("offset", offset), ("valid_end", valid_end), ("sequence", sequence)]
            {
                if let Err(error) = value.setattr(name, number) {
                    return error;
                }
            }
            if let Err(error) = value.setattr("reason", reason) {
                return error;
            }
            exception
        }),
        BookError::SaveUncertain(_) => NativeBookSaveUncertainError::new_err(error.to_string()),
        _ => PyValueError::new_err(error.to_string()),
    }
}

/// A bound on a native book score.
#[pyclass(eq, eq_int, from_py_object, name = "NativeBound")]
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum PyNativeBound {
    Exact,
    Lower,
    Upper,
}
impl From<PyNativeBound> for NativeBound {
    fn from(value: PyNativeBound) -> Self {
        match value {
            PyNativeBound::Exact => Self::Exact,
            PyNativeBound::Lower => Self::Lower,
            PyNativeBound::Upper => Self::Upper,
        }
    }
}
impl From<NativeBound> for PyNativeBound {
    fn from(value: NativeBound) -> Self {
        match value {
            NativeBound::Exact => Self::Exact,
            NativeBound::Lower => Self::Lower,
            NativeBound::Upper => Self::Upper,
        }
    }
}

/// A score from the side-to-move perspective, before the candidate move.
#[pyclass(from_py_object, name = "NativeScore")]
#[derive(Clone)]
pub(crate) struct PyNativeScore {
    inner: NativeScore,
}
#[pymethods]
impl PyNativeScore {
    #[staticmethod]
    fn unknown() -> Self {
        Self { inner: NativeScore::Unknown }
    }
    #[staticmethod]
    #[pyo3(signature = (value, bound=PyNativeBound::Exact))]
    fn cp(value: i32, bound: PyNativeBound) -> Self {
        Self { inner: NativeScore::Cp { value, bound: bound.into() } }
    }
    #[staticmethod]
    #[pyo3(signature = (plies, bound=PyNativeBound::Exact))]
    fn mate(plies: i32, bound: PyNativeBound) -> Self {
        Self { inner: NativeScore::Mate { plies, bound: bound.into() } }
    }
    #[staticmethod]
    fn mate_unknown(winning: bool) -> Self {
        Self { inner: NativeScore::MateUnknown { winning } }
    }
    #[getter]
    fn kind(&self) -> &'static str {
        match self.inner {
            NativeScore::Unknown => "unknown",
            NativeScore::Cp { .. } => "cp",
            NativeScore::Mate { .. } => "mate",
            NativeScore::MateUnknown { .. } => "mate_unknown",
        }
    }
    #[getter]
    fn value(&self) -> Option<i32> {
        match self.inner {
            NativeScore::Cp { value, .. } => Some(value),
            NativeScore::Mate { plies, .. } => Some(plies),
            _ => None,
        }
    }
    #[getter]
    fn bound(&self) -> Option<PyNativeBound> {
        match self.inner {
            NativeScore::Cp { bound, .. } | NativeScore::Mate { bound, .. } => Some(bound.into()),
            _ => None,
        }
    }
    #[getter]
    fn winning(&self) -> Option<bool> {
        match self.inner {
            NativeScore::MateUnknown { winning } => Some(winning),
            _ => None,
        }
    }
}

/// A candidate move with optional depth and display order.
#[pyclass(from_py_object, name = "NativeCandidate")]
#[derive(Clone)]
pub(crate) struct PyNativeCandidate {
    inner: NativeCandidate,
}
#[pymethods]
impl PyNativeCandidate {
    #[new]
    #[pyo3(signature = (mv, score=None, depth=None, display_order=0))]
    fn new(
        mv: &PyMove,
        score: Option<&PyNativeScore>,
        depth: Option<u32>,
        display_order: u16,
    ) -> Self {
        Self {
            inner: NativeCandidate {
                mv: mv.inner,
                score: score.map_or(NativeScore::Unknown, |value| value.inner),
                depth,
                display_order,
            },
        }
    }
    #[getter]
    fn mv(&self) -> PyMove {
        PyMove { inner: self.inner.mv }
    }
    #[getter]
    fn score(&self) -> PyNativeScore {
        PyNativeScore { inner: self.inner.score }
    }
    #[getter]
    fn depth(&self) -> Option<u32> {
        self.inner.depth
    }
    #[getter]
    fn display_order(&self) -> u16 {
        self.inner.display_order
    }
}

/// Optional title and description, and the meaning of candidate depth values.
#[pyclass(from_py_object, name = "NativeBookMetadata")]
#[derive(Clone)]
pub(crate) struct PyNativeBookMetadata {
    inner: NativeBookMetadata,
}
#[pymethods]
impl PyNativeBookMetadata {
    #[new]
    #[pyo3(signature = (title=None, description=None, depth_kind=0))]
    fn new(title: Option<String>, description: Option<String>, depth_kind: u32) -> Self {
        Self { inner: NativeBookMetadata { title, description, depth_kind } }
    }
    #[getter]
    fn title(&self) -> Option<String> {
        self.inner.title.clone()
    }
    #[getter]
    fn description(&self) -> Option<String> {
        self.inner.description.clone()
    }
    /// Depth semantics: 0 unspecified, 1 search, 2 backward; other u32 values are preserved.
    #[getter]
    fn depth_kind(&self) -> u32 {
        self.inner.depth_kind
    }
}

/// A canonical position and candidates in move integer order.
#[pyclass(from_py_object, name = "NativeBookEntry")]
#[derive(Clone)]
pub(crate) struct PyNativeBookEntry {
    inner: NativeBookEntry,
}
#[pymethods]
impl PyNativeBookEntry {
    #[new]
    #[pyo3(signature = (board, candidates=Vec::new()))]
    fn new(board: &PyBoard, candidates: Vec<PyNativeCandidate>) -> PyResult<Self> {
        Ok(Self {
            inner: NativeBookEntry::from_position(
                &board.position,
                candidates.into_iter().map(|candidate| candidate.inner).collect(),
            )
            .map_err(error)?,
        })
    }
    #[getter]
    fn key<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.key)
    }
    #[getter]
    fn candidates(&self) -> Vec<PyNativeCandidate> {
        self.inner.candidates.iter().cloned().map(|inner| PyNativeCandidate { inner }).collect()
    }
    fn position(&self) -> PyResult<PyBoard> {
        Ok(PyBoard { position: Box::new(self.inner.position().map_err(error)?) })
    }
}

/// An owned position snapshot including comments keyed by raw move integer.
#[pyclass(from_py_object, name = "NativeBookPosition")]
#[derive(Clone)]
pub(crate) struct PyNativeBookPosition {
    inner: NativeBookPosition,
}
#[pymethods]
impl PyNativeBookPosition {
    #[new]
    #[pyo3(signature = (entry, comment=None, move_comments=None))]
    fn new(
        entry: &PyNativeBookEntry,
        comment: Option<String>,
        move_comments: Option<BTreeMap<u16, String>>,
    ) -> Self {
        Self {
            inner: NativeBookPosition {
                entry: entry.inner.clone(),
                comment,
                move_comments: move_comments.unwrap_or_default(),
            },
        }
    }
    #[getter]
    fn entry(&self) -> PyNativeBookEntry {
        PyNativeBookEntry { inner: self.inner.entry.clone() }
    }
    #[getter]
    fn comment(&self) -> Option<String> {
        self.inner.comment.clone()
    }
    #[getter]
    fn move_comments(&self) -> BTreeMap<u16, String> {
        self.inner.move_comments.clone()
    }
}

/// A file-backed native book reader. Opening does not imply full validation.
#[pyclass(name = "NativeBook")]
pub(crate) struct PyNativeBook {
    inner: NativeBook,
}
#[pymethods]
impl PyNativeBook {
    #[staticmethod]
    fn open(path: PathBuf) -> PyResult<Self> {
        Ok(Self { inner: NativeBook::open(path).map_err(error)? })
    }
    /// Number of positions per compressed block stored in this file.
    #[getter]
    fn block_positions(&self) -> u32 {
        self.inner.block_positions()
    }
    fn get(&self, board: &PyBoard) -> PyResult<Option<PyNativeBookEntry>> {
        Ok(self.inner.get(&board.position).map_err(error)?.map(|inner| PyNativeBookEntry { inner }))
    }
    fn record(&self, board: &PyBoard) -> PyResult<Option<PyNativeBookPosition>> {
        Ok(self
            .inner
            .record(&board.position)
            .map_err(error)?
            .map(|inner| PyNativeBookPosition { inner }))
    }
    fn position_comment(&self, board: &PyBoard) -> PyResult<Option<String>> {
        self.inner.position_comment(&board.position).map_err(error)
    }
    fn move_comment(&self, board: &PyBoard, mv: &PyMove) -> PyResult<Option<String>> {
        self.inner.move_comment(&board.position, mv.inner).map_err(error)
    }
    fn metadata(&self) -> PyResult<PyNativeBookMetadata> {
        Ok(PyNativeBookMetadata { inner: self.inner.metadata().map_err(error)? })
    }
    fn entries(&self) -> PyResult<Vec<PyNativeBookEntry>> {
        self.inner
            .entries()
            .map(|entry| entry.map(|inner| PyNativeBookEntry { inner }).map_err(error))
            .collect()
    }
    fn records(&self) -> PyResult<Vec<PyNativeBookPosition>> {
        self.inner
            .records()
            .map(|record| record.map(|inner| PyNativeBookPosition { inner }).map_err(error))
            .collect()
    }
    fn __len__(&self) -> PyResult<usize> {
        usize::try_from(self.inner.len().map_err(error)?)
            .map_err(|_| PyOverflowError::new_err("native book length exceeds platform limit"))
    }
    fn validate_full(&self) -> PyResult<()> {
        self.inner.validate_full().map_err(error)
    }
    fn tail(&self) -> Option<(u64, u64)> {
        self.inner.tail().map(|tail| (tail.offset, tail.end))
    }
    fn io_stats(&self) -> (u64, u64) {
        let stats = self.inner.io_stats();
        (stats.read_calls, stats.read_bytes)
    }
}

/// A streaming builder for a new native book file.
#[pyclass(name = "NativeBookBuilder")]
pub(crate) struct PyNativeBookBuilder {
    inner: Option<NativeBookBuilder>,
}
#[pymethods]
impl PyNativeBookBuilder {
    #[new]
    #[pyo3(signature = (path, metadata=None, *, block_positions=64))]
    fn new(
        path: PathBuf,
        metadata: Option<&PyNativeBookMetadata>,
        block_positions: u32,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: Some(
                NativeBookBuilder::new(
                    path,
                    metadata.map_or_else(NativeBookMetadata::default, |meta| meta.inner.clone()),
                )
                .map_err(error)?
                .with_block_positions(block_positions)
                .map_err(error)?,
            ),
        })
    }
    fn push(&mut self, record: &PyNativeBookPosition) -> PyResult<()> {
        self.inner
            .as_mut()
            .ok_or_else(|| PyRuntimeError::new_err("native builder is finished"))?
            .push(record.inner.clone())
            .map_err(error)
    }
    fn finish(&mut self) -> PyResult<()> {
        self.inner
            .take()
            .ok_or_else(|| PyRuntimeError::new_err("native builder is finished"))?
            .finish()
            .map_err(error)
    }
}

/// An exclusive editor that keeps changes dirty until save succeeds.
#[pyclass(name = "NativeBookEditor")]
pub(crate) struct PyNativeBookEditor {
    inner: Option<NativeBookEditor>,
}
impl PyNativeBookEditor {
    fn inner(&self) -> PyResult<&NativeBookEditor> {
        self.inner.as_ref().ok_or_else(|| PyRuntimeError::new_err("native editor is closed"))
    }
    fn inner_mut(&mut self) -> PyResult<&mut NativeBookEditor> {
        self.inner.as_mut().ok_or_else(|| PyRuntimeError::new_err("native editor is closed"))
    }
}
#[pymethods]
impl PyNativeBookEditor {
    #[staticmethod]
    #[pyo3(signature = (path, metadata=None))]
    fn create(path: PathBuf, metadata: Option<&PyNativeBookMetadata>) -> PyResult<Self> {
        Ok(Self {
            inner: Some(
                NativeBookEditor::create(
                    path,
                    metadata.map_or_else(NativeBookMetadata::default, |meta| meta.inner.clone()),
                )
                .map_err(error)?,
            ),
        })
    }
    #[staticmethod]
    fn open(path: PathBuf) -> PyResult<Self> {
        Ok(Self { inner: Some(NativeBookEditor::open(path).map_err(error)?) })
    }
    fn close(&mut self) {
        self.inner = None;
    }
    fn record(&self, board: &PyBoard) -> PyResult<Option<PyNativeBookPosition>> {
        Ok(self
            .inner()?
            .record(&board.position)
            .map_err(error)?
            .map(|inner| PyNativeBookPosition { inner }))
    }
    fn metadata(&self) -> PyResult<PyNativeBookMetadata> {
        Ok(PyNativeBookMetadata { inner: self.inner()?.metadata().map_err(error)? })
    }
    fn upsert_position(&mut self, record: &PyNativeBookPosition) -> PyResult<()> {
        self.inner_mut()?.upsert_position(record.inner.clone()).map_err(error)
    }
    #[pyo3(signature = (board, candidate, comment=None))]
    fn upsert_candidate(
        &mut self,
        board: &PyBoard,
        candidate: &PyNativeCandidate,
        comment: Option<String>,
    ) -> PyResult<()> {
        self.inner_mut()?
            .upsert_candidate(&board.position, candidate.inner.clone(), comment)
            .map_err(error)
    }
    fn remove_position(&mut self, board: &PyBoard) -> PyResult<bool> {
        self.inner_mut()?.remove_position(&board.position).map_err(error)
    }
    fn remove_candidate(&mut self, board: &PyBoard, mv: &PyMove) -> PyResult<bool> {
        self.inner_mut()?.remove_candidate(&board.position, mv.inner).map_err(error)
    }
    fn reorder_candidate(&mut self, board: &PyBoard, mv: &PyMove, target: u16) -> PyResult<bool> {
        self.inner_mut()?.reorder_candidate(&board.position, mv.inner, target).map_err(error)
    }
    fn set_position_comment(&mut self, board: &PyBoard, comment: Option<String>) -> PyResult<()> {
        self.inner_mut()?.set_position_comment(&board.position, comment).map_err(error)
    }
    fn set_metadata(&mut self, metadata: &PyNativeBookMetadata) -> PyResult<()> {
        self.inner_mut()?.set_metadata(metadata.inner.clone()).map_err(error)
    }
    fn save(&mut self) -> PyResult<bool> {
        self.inner_mut()?.save().map_err(error)
    }
    fn compact(&mut self) -> PyResult<()> {
        self.inner_mut()?.compact().map_err(error)
    }
    fn is_dirty(&self) -> PyResult<bool> {
        Ok(self.inner()?.is_dirty())
    }
    #[staticmethod]
    fn recover_tail(path: PathBuf) -> PyResult<PathBuf> {
        NativeBookEditor::recover_tail(path).map_err(error)
    }
    #[staticmethod]
    fn salvage_prefix(path: PathBuf, output: PathBuf) -> PyResult<PyNativeBookSalvageReport> {
        Ok(PyNativeBookSalvageReport {
            inner: NativeBookEditor::salvage_prefix(path, output).map_err(error)?,
        })
    }
}

/// Diagnostics for a validated prefix recovered to a new file.
#[pyclass(name = "NativeBookSalvageReport")]
pub(crate) struct PyNativeBookSalvageReport {
    inner: NativeBookSalvageReport,
}
#[pymethods]
impl PyNativeBookSalvageReport {
    #[getter]
    fn source_bytes(&self) -> u64 {
        self.inner.source_bytes
    }
    #[getter]
    fn valid_end(&self) -> u64 {
        self.inner.valid_end
    }
    #[getter]
    fn sequence(&self) -> u64 {
        self.inner.sequence
    }
    #[getter]
    fn error_offset(&self) -> u64 {
        self.inner.error_offset
    }
    #[getter]
    fn output(&self) -> PathBuf {
        self.inner.output.clone()
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("NativeBookLogError", m.py().get_type::<NativeBookLogError>())?;
    m.add("NativeBookSaveUncertainError", m.py().get_type::<NativeBookSaveUncertainError>())?;
    m.add_class::<PyNativeBound>()?;
    m.add_class::<PyNativeScore>()?;
    m.add_class::<PyNativeCandidate>()?;
    m.add_class::<PyNativeBookMetadata>()?;
    m.add_class::<PyNativeBookEntry>()?;
    m.add_class::<PyNativeBookPosition>()?;
    m.add_class::<PyNativeBook>()?;
    m.add_class::<PyNativeBookBuilder>()?;
    m.add_class::<PyNativeBookEditor>()?;
    m.add_class::<PyNativeBookSalvageReport>()?;
    Ok(())
}
