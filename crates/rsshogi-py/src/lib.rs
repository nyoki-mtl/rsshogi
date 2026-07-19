// Python バインディングは薄い互換レイヤーである。PyO3 のメソッド名、クラス属性、
// Python 向けの変換名は、Rust/Clippy の pedantic 規約に意図的に従わない場合がある。
#![allow(clippy::pedantic, clippy::nursery)]

use pyo3::basic::CompareOp;
use pyo3::exceptions::{PyIndexError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBool, PyBytes, PyDict, PyInt, PyList, PyType};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

mod dtype;
mod book;
mod pack;
mod policy;
mod python_types;
mod sazpack;
#[cfg(all(test, feature = "python-tests"))]
pub(crate) use book::{
    PyBookMove, PyMemoryBook, PyStaticBook, book_key_after, book_key_from_position,
};
use dtype::add_dtype_module;
use python_types::{
    PyBitboard, PyColor, PyHand, PyMoveType, PyPiece, PyPieceType, PyRepetitionState, PySquare,
};

use ::rsshogi::board::movegen::{Evasions, NonEvasions, generate_legal_all, generate_moves};
use ::rsshogi::board::position::{
    DeclarationDetail, MoveDelta32, ValidationError as RawValidationError,
};
use ::rsshogi::board::{
    self, BoardArray, HuffmanCodedPos, InitialPosition, MoveList, PackedSfen, Position,
    PositionState as RawPositionState, ValidationIssue as RawValidationIssue,
    ValidationReport as RawValidationReport,
};
#[cfg(all(test, feature = "python-tests"))]
use ::rsshogi::book::{Book, BookMove};
use ::rsshogi::mate;
use ::rsshogi::records::formats::sbinpack::{
    SbinpackMetadata, chains_from_record_with_metadata as chains_from_record_sbinpack,
    deserialize_file as deserialize_sbinpack_file, serialize_file as serialize_sbinpack_file,
    unpack_ply_result,
};
use ::rsshogi::records::formats::{
    HcpeGameResult, HuffmanCodedPosAndEval, UsiPositionExportOptions, UsiPositionParseOptions,
    board_to_bod, board_to_csa, encode_entry as encode_hcpe_entry, encode_game as encode_game_pack,
    export_csa as export_csa_record, export_jkf as export_jkf_record,
    export_ki2 as export_ki2_record, export_kif as export_kif_record,
    export_usi_position_with_options as export_usi_position_record,
    game_from_record as game_from_record_pack, move_to_bod, parse_csa_str as parse_csa_str_record,
    parse_jkf_str as parse_jkf_str_record, parse_ki2_str as parse_ki2_str_record,
    parse_kif_str as parse_kif_str_record,
    parse_usi_position_with_options as parse_usi_position_record,
    records_from_bytes as records_from_bytes_pack,
};
use ::rsshogi::records::record::{
    EngineExtraValue, EngineInfo, MoveEntry, Record, RecordAnnotation, RecordEditor, RecordEntry,
    RecordInitialPosition, RecordMetadata, RecordNodeId, SpecialMove, SpecialMoveEntry,
};
use ::rsshogi::records::time_control::TimeControl;
use ::rsshogi::types::{
    AperyMove, AperyMove32, Color, Eval, GameResult, Hand, HandPiece, MOVE_END, MOVE_NONE,
    MOVE_NULL, MOVE_RESIGN, MOVE_WIN, Move, Move32, Piece, Square,
};

#[pyclass(name = "Move")]
#[derive(Clone, Copy)]
struct PyMove {
    inner: Move,
}

#[pymethods]
#[allow(dead_code)]
impl PyMove {
    #[new]
    fn new(value: u16) -> Self {
        Self { inner: Move::from_raw(value) }
    }

    #[classmethod]
    fn from_usi(_cls: &Bound<'_, PyType>, usi: &str) -> PyResult<Self> {
        Move::from_usi(usi)
            .filter(|mv| mv.is_normal())
            .map(|mv| Self { inner: mv })
            .ok_or_else(|| PyValueError::new_err("invalid USI move"))
    }

    #[classattr]
    #[pyo3(name = "MOVE_NONE")]
    fn move_none(py: Python<'_>) -> PyResult<Py<PyMove>> {
        Py::new(py, Self { inner: Move::MOVE_NONE })
    }

    #[classattr]
    #[pyo3(name = "MOVE_NULL")]
    fn move_null(py: Python<'_>) -> PyResult<Py<PyMove>> {
        Py::new(py, Self { inner: Move::MOVE_NULL })
    }

    #[classattr]
    #[pyo3(name = "MOVE_RESIGN")]
    fn move_resign(py: Python<'_>) -> PyResult<Py<PyMove>> {
        Py::new(py, Self { inner: Move::MOVE_RESIGN })
    }

    #[classattr]
    #[pyo3(name = "MOVE_WIN")]
    fn move_win(py: Python<'_>) -> PyResult<Py<PyMove>> {
        Py::new(py, Self { inner: Move::MOVE_WIN })
    }

    #[classattr]
    #[pyo3(name = "MOVE_END")]
    fn move_end(py: Python<'_>) -> PyResult<Py<PyMove>> {
        Py::new(py, Self { inner: Move::MOVE_END })
    }

    #[allow(clippy::wrong_self_convention)]
    fn to_usi(&self) -> String {
        self.inner.to_usi()
    }

    #[allow(clippy::wrong_self_convention)]
    fn to_ki2(&self, board: &PyBoard) -> PyResult<Option<String>> {
        let mv = board.position.move32_from_move(self.inner);
        if !mv.is_normal() {
            return Err(PyValueError::new_err("invalid move value"));
        }
        Ok(mv.to_ki2(&board.position))
    }

    fn is_drop(&self) -> bool {
        self.inner.is_drop()
    }

    fn is_promotion(&self) -> bool {
        self.inner.is_promotion()
    }

    fn is_normal(&self) -> bool {
        self.inner.is_normal()
    }

    #[allow(clippy::wrong_self_convention)]
    #[getter]
    fn from_sq(&self) -> PySquare {
        PySquare::from_square(self.inner.from_sq())
    }

    #[allow(clippy::wrong_self_convention)]
    #[getter]
    fn to_sq(&self) -> PySquare {
        PySquare::from_square(self.inner.to_sq())
    }

    #[getter]
    fn dropped_piece_type(&self) -> Option<PyPieceType> {
        self.inner.dropped_piece().map(PyPieceType::from_piece_type)
    }

    #[getter]
    fn move_type(&self) -> PyMoveType {
        PyMoveType::from_move_type(self.inner.move_type())
    }

    #[getter]
    fn value(&self) -> u16 {
        self.inner.raw()
    }

    #[allow(clippy::wrong_self_convention)]
    fn to_apery(&self) -> PyAperyMove {
        PyAperyMove { inner: self.inner.to_apery() }
    }

    fn __int__(&self) -> u16 {
        self.inner.raw()
    }

    fn __repr__(&self) -> String {
        format!("Move({})", self.inner.to_usi())
    }

    fn __str__(&self) -> String {
        self.inner.to_usi()
    }

    fn __hash__(&self) -> i64 {
        i64::from(self.inner.raw())
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        let other_value = match other.extract::<PyRef<'_, PyMove>>() {
            Ok(other) => other.inner.raw(),
            _ => match other.extract::<u16>() {
                Ok(value) => value,
                _ => {
                    return Err(PyTypeError::new_err("unsupported comparison for Move"));
                }
            },
        };
        match op {
            CompareOp::Eq => Ok(self.inner.raw() == other_value),
            CompareOp::Ne => Ok(self.inner.raw() != other_value),
            _ => Err(PyTypeError::new_err("Move only supports == and !=")),
        }
    }
}

impl PyMove {
    pub(crate) fn from_move(inner: Move) -> Self {
        Self { inner }
    }

    #[allow(dead_code)]
    pub(crate) fn inner(&self) -> Move {
        self.inner
    }
}

#[pyclass(name = "AperyMove")]
#[derive(Clone, Copy)]
struct PyAperyMove {
    inner: AperyMove,
}

#[pymethods]
impl PyAperyMove {
    #[new]
    fn new(value: u16) -> Self {
        Self { inner: AperyMove::from_raw(value) }
    }

    #[classmethod]
    fn from_usi(_cls: &Bound<'_, PyType>, usi: &str) -> PyResult<Self> {
        AperyMove::from_usi(usi)
            .map(|mv| Self { inner: mv })
            .ok_or_else(|| PyValueError::new_err("invalid USI move"))
    }

    #[allow(clippy::wrong_self_convention)]
    fn to_usi(&self) -> String {
        self.inner.to_usi()
    }

    #[allow(clippy::wrong_self_convention)]
    fn to_move(&self) -> PyMove {
        PyMove::from_move(self.inner.to_move())
    }

    fn is_drop(&self) -> bool {
        self.inner.is_drop()
    }

    fn is_promotion(&self) -> bool {
        self.inner.is_promotion()
    }

    fn is_normal(&self) -> bool {
        self.inner.is_normal()
    }

    #[allow(clippy::wrong_self_convention)]
    #[getter]
    fn from_sq(&self) -> PySquare {
        PySquare::from_square(self.inner.from_sq())
    }

    #[allow(clippy::wrong_self_convention)]
    #[getter]
    fn to_sq(&self) -> PySquare {
        PySquare::from_square(self.inner.to_sq())
    }

    #[getter]
    fn dropped_piece_type(&self) -> Option<PyPieceType> {
        self.inner.dropped_piece().map(PyPieceType::from_piece_type)
    }

    #[getter]
    fn move_type(&self) -> PyMoveType {
        PyMoveType::from_move_type(self.inner.move_type())
    }

    #[getter]
    fn value(&self) -> u16 {
        self.inner.raw()
    }

    fn __int__(&self) -> u16 {
        self.inner.raw()
    }

    fn __repr__(&self) -> String {
        format!("AperyMove({})", self.inner.to_usi())
    }

    fn __str__(&self) -> String {
        self.inner.to_usi()
    }

    fn __hash__(&self) -> i64 {
        i64::from(self.inner.raw())
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        let other_value = match other.extract::<PyRef<'_, PyAperyMove>>() {
            Ok(other) => other.inner.raw(),
            _ => match other.extract::<u16>() {
                Ok(value) => value,
                _ => {
                    return Err(PyTypeError::new_err("unsupported comparison for AperyMove"));
                }
            },
        };
        match op {
            CompareOp::Eq => Ok(self.inner.raw() == other_value),
            CompareOp::Ne => Ok(self.inner.raw() != other_value),
            _ => Err(PyTypeError::new_err("AperyMove only supports == and !=")),
        }
    }
}

#[pyclass(name = "AperyMove32")]
#[derive(Clone, Copy)]
struct PyAperyMove32 {
    inner: AperyMove32,
}

#[pymethods]
impl PyAperyMove32 {
    #[new]
    fn new(value: u32) -> Self {
        Self { inner: AperyMove32::from_raw(value) }
    }

    #[allow(clippy::wrong_self_convention)]
    fn to_usi(&self) -> String {
        self.inner.to_usi()
    }

    #[allow(clippy::wrong_self_convention)]
    fn to_move(&self) -> PyMove {
        PyMove::from_move(self.inner.to_move())
    }

    #[allow(clippy::wrong_self_convention)]
    fn to_move32(&self, board: &PyBoard) -> PyMove32 {
        PyMove32 { inner: board.position.move32_from_apery_move32(self.inner) }
    }

    #[allow(clippy::wrong_self_convention)]
    fn to_apery_move(&self) -> PyAperyMove {
        PyAperyMove { inner: self.inner.to_apery_move() }
    }

    fn is_drop(&self) -> bool {
        self.inner.is_drop()
    }

    fn is_promotion(&self) -> bool {
        self.inner.is_promotion()
    }

    fn is_capture(&self) -> bool {
        self.inner.is_capture()
    }

    fn is_normal(&self) -> bool {
        self.inner.is_normal()
    }

    #[allow(clippy::wrong_self_convention)]
    #[getter]
    fn from_sq(&self) -> PySquare {
        PySquare::from_square(self.inner.from_sq())
    }

    #[allow(clippy::wrong_self_convention)]
    #[getter]
    fn to_sq(&self) -> PySquare {
        PySquare::from_square(self.inner.to_sq())
    }

    #[getter]
    fn dropped_piece_type(&self) -> Option<PyPieceType> {
        self.inner.dropped_piece().map(PyPieceType::from_piece_type)
    }

    #[getter]
    fn move_type(&self) -> PyMoveType {
        PyMoveType::from_move_type(self.inner.move_type())
    }

    #[getter]
    fn piece_type_before(&self) -> PyPieceType {
        PyPieceType::from_piece_type(self.inner.piece_type_before())
    }

    #[getter]
    fn piece_type_after(&self) -> PyPieceType {
        PyPieceType::from_piece_type(self.inner.piece_type_after())
    }

    #[getter]
    fn captured_piece_type(&self) -> PyPieceType {
        PyPieceType::from_piece_type(self.inner.captured_piece_type())
    }

    #[getter]
    fn value(&self) -> u32 {
        self.inner.raw()
    }

    fn __int__(&self) -> u32 {
        self.inner.raw()
    }

    fn __repr__(&self) -> String {
        format!("AperyMove32({})", self.inner.to_usi())
    }

    fn __str__(&self) -> String {
        self.inner.to_usi()
    }

    fn __hash__(&self) -> i64 {
        i64::from(self.inner.raw())
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        let other_value = match other.extract::<PyRef<'_, PyAperyMove32>>() {
            Ok(other) => other.inner.raw(),
            _ => match other.extract::<u32>() {
                Ok(value) => value,
                _ => {
                    return Err(PyTypeError::new_err("unsupported comparison for AperyMove32"));
                }
            },
        };
        match op {
            CompareOp::Eq => Ok(self.inner.raw() == other_value),
            CompareOp::Ne => Ok(self.inner.raw() != other_value),
            _ => Err(PyTypeError::new_err("AperyMove32 only supports == and !=")),
        }
    }
}

#[pyclass(name = "Move32")]
#[derive(Clone, Copy)]
struct PyMove32 {
    inner: Move32,
}

#[pymethods]
impl PyMove32 {
    #[new]
    fn new(value: u32) -> Self {
        Self { inner: Move32::from_raw(value) }
    }

    #[classmethod]
    fn from_usi(_cls: &Bound<'_, PyType>, usi: &str) -> PyResult<Self> {
        Move32::from_usi(usi)
            .filter(|mv| mv.is_normal())
            .map(|mv| Self { inner: mv })
            .ok_or_else(|| PyValueError::new_err("invalid USI move"))
    }

    #[classattr]
    #[pyo3(name = "MOVE_NONE")]
    fn move_none(py: Python<'_>) -> PyResult<Py<PyMove32>> {
        Py::new(py, Self { inner: MOVE_NONE })
    }

    #[classattr]
    #[pyo3(name = "MOVE_NULL")]
    fn move_null(py: Python<'_>) -> PyResult<Py<PyMove32>> {
        Py::new(py, Self { inner: MOVE_NULL })
    }

    #[classattr]
    #[pyo3(name = "MOVE_RESIGN")]
    fn move_resign(py: Python<'_>) -> PyResult<Py<PyMove32>> {
        Py::new(py, Self { inner: MOVE_RESIGN })
    }

    #[classattr]
    #[pyo3(name = "MOVE_WIN")]
    fn move_win(py: Python<'_>) -> PyResult<Py<PyMove32>> {
        Py::new(py, Self { inner: MOVE_WIN })
    }

    #[classattr]
    #[pyo3(name = "MOVE_END")]
    fn move_end(py: Python<'_>) -> PyResult<Py<PyMove32>> {
        Py::new(py, Self { inner: MOVE_END })
    }

    #[allow(clippy::wrong_self_convention)]
    fn to_usi(&self) -> String {
        self.inner.to_usi()
    }

    fn is_drop(&self) -> bool {
        self.inner.is_drop()
    }

    fn is_promotion(&self) -> bool {
        self.inner.is_promotion()
    }

    fn is_normal(&self) -> bool {
        self.inner.is_normal()
    }

    #[allow(clippy::wrong_self_convention)]
    #[getter]
    fn from_sq(&self) -> PySquare {
        PySquare::from_square(self.inner.from_sq())
    }

    #[allow(clippy::wrong_self_convention)]
    #[getter]
    fn to_sq(&self) -> PySquare {
        PySquare::from_square(self.inner.to_sq())
    }

    #[getter]
    fn dropped_piece_type(&self) -> Option<PyPieceType> {
        self.inner.dropped_piece().map(PyPieceType::from_piece_type)
    }

    #[getter]
    fn move_type(&self) -> PyMoveType {
        PyMoveType::from_move_type(self.inner.move_type())
    }

    #[getter]
    fn piece_after_move(&self) -> PyPiece {
        PyPiece::from_piece(self.inner.piece_after_move())
    }

    #[allow(clippy::wrong_self_convention)]
    fn to_move(&self) -> PyMove {
        PyMove::from_move(self.inner.to_move())
    }

    #[allow(clippy::wrong_self_convention)]
    fn to_apery(&self, board: &PyBoard) -> PyAperyMove32 {
        PyAperyMove32 { inner: board.position.apery_move32_from_move32(self.inner) }
    }

    #[allow(clippy::wrong_self_convention)]
    fn to_csa(&self) -> Option<String> {
        self.inner.to_csa()
    }

    #[allow(clippy::wrong_self_convention)]
    fn to_ki2(&self, board: &PyBoard) -> Option<String> {
        self.inner.to_ki2(&board.position)
    }

    #[getter]
    fn value(&self) -> u32 {
        self.inner.raw()
    }

    fn __int__(&self) -> u32 {
        self.inner.raw()
    }

    fn __repr__(&self) -> String {
        format!("Move32({})", self.inner.to_usi())
    }

    fn __str__(&self) -> String {
        self.inner.to_usi()
    }

    fn __hash__(&self) -> i64 {
        i64::from(self.inner.raw())
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        let other_value = match other.extract::<PyRef<'_, PyMove32>>() {
            Ok(other) => other.inner.raw(),
            _ => match other.extract::<u32>() {
                Ok(value) => value,
                _ => {
                    return Err(PyTypeError::new_err("unsupported comparison for Move32"));
                }
            },
        };
        match op {
            CompareOp::Eq => Ok(self.inner.raw() == other_value),
            CompareOp::Ne => Ok(self.inner.raw() != other_value),
            _ => Err(PyTypeError::new_err("Move32 only supports == and !=")),
        }
    }
}

#[pyclass(name = "GameResult")]
#[derive(Clone, Copy)]
pub(crate) struct PyGameResult {
    pub(crate) inner: GameResult,
}

const GAME_RESULT_MEMBER_NAMES: &[&str] = &[
    "BLACK_WIN",
    "WHITE_WIN",
    "DRAW_BY_REPETITION",
    "ERROR",
    "BLACK_WIN_BY_DECLARATION",
    "WHITE_WIN_BY_DECLARATION",
    "DRAW_BY_MAX_PLIES",
    "INVALID",
    "BLACK_WIN_BY_FORFEIT",
    "WHITE_WIN_BY_FORFEIT",
    "DRAW_BY_IMPASSE",
    "BLACK_WIN_BY_TRY_RULE",
    "WHITE_WIN_BY_TRY_RULE",
    "BLACK_WIN_BY_ILLEGAL_MOVE",
    "WHITE_WIN_BY_ILLEGAL_MOVE",
    "PAUSED",
    "BLACK_WIN_BY_TIMEOUT",
    "WHITE_WIN_BY_TIMEOUT",
];

#[pyclass(name = "GameResultInfo")]
#[derive(Clone)]
struct PyGameResultInfo {
    result: GameResult,
    ply_count: usize,
    reason: Option<String>,
    end_time_ms: Option<u32>,
    end_comment: Option<String>,
}

#[pymethods]
impl PyGameResultInfo {
    #[getter]
    fn result(&self) -> PyGameResult {
        PyGameResult { inner: self.result }
    }

    #[getter]
    fn ply_count(&self) -> usize {
        self.ply_count
    }

    #[getter]
    fn reason(&self) -> Option<String> {
        self.reason.clone()
    }

    #[getter]
    fn end_time_ms(&self) -> Option<u32> {
        self.end_time_ms
    }

    #[getter]
    fn end_comment(&self) -> Option<String> {
        self.end_comment.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "GameResultInfo(result={}, ply_count={}, end_time_ms={:?})",
            game_result_name(self.result),
            self.ply_count,
            self.end_time_ms
        )
    }
}

#[pyclass(name = "TimeControl")]
#[derive(Clone, Copy)]
struct PyTimeControl {
    inner: TimeControl,
}

#[pymethods]
impl PyTimeControl {
    #[new]
    fn new(base_seconds: u32, byoyomi_seconds: u32, increment_seconds: u32) -> Self {
        Self { inner: TimeControl::new(base_seconds, byoyomi_seconds, increment_seconds) }
    }

    #[getter]
    fn base_seconds(&self) -> u32 {
        self.inner.base_seconds()
    }

    #[getter]
    fn byoyomi_seconds(&self) -> u32 {
        self.inner.byoyomi_seconds()
    }

    #[getter]
    fn increment_seconds(&self) -> u32 {
        self.inner.increment_seconds()
    }

    #[allow(clippy::wrong_self_convention)]
    fn to_spec(&self) -> String {
        self.inner.to_spec()
    }

    #[classmethod]
    fn from_spec(_cls: &Bound<'_, PyType>, spec: &str) -> PyResult<Self> {
        TimeControl::from_spec(spec)
            .map(|inner| Self { inner })
            .ok_or_else(|| PyValueError::new_err("invalid time control spec"))
    }

    #[classmethod]
    fn normalize_spec(_cls: &Bound<'_, PyType>, spec: &str) -> PyResult<String> {
        TimeControl::normalize_spec(spec)
            .ok_or_else(|| PyValueError::new_err("invalid time control spec"))
    }

    fn __repr__(&self) -> String {
        format!(
            "TimeControl(base_seconds={}, byoyomi_seconds={}, increment_seconds={})",
            self.inner.base_seconds(),
            self.inner.byoyomi_seconds(),
            self.inner.increment_seconds()
        )
    }
}

#[pymethods]
#[allow(non_snake_case)]
impl PyGameResult {
    #[new]
    fn new(value: u8) -> PyResult<Self> {
        let inner = game_result_from_value(value)
            .ok_or_else(|| PyValueError::new_err("invalid GameResult value"))?;
        Ok(Self { inner })
    }

    #[getter]
    fn value(&self) -> u8 {
        self.inner as u8
    }

    #[getter]
    fn name(&self) -> &'static str {
        game_result_name(self.inner)
    }

    fn is_black_win(&self) -> bool {
        self.inner.is_black_win()
    }

    fn is_white_win(&self) -> bool {
        self.inner.is_white_win()
    }

    fn is_draw(&self) -> bool {
        self.inner.is_draw()
    }

    fn is_win(&self) -> bool {
        self.inner.is_win()
    }

    fn is_win_by_declaration(&self) -> bool {
        self.inner.is_win_by_declaration()
    }

    fn is_win_by_try_rule(&self) -> bool {
        self.inner.is_win_by_try_rule()
    }

    fn __int__(&self) -> u8 {
        self.inner as u8
    }

    fn __hash__(&self) -> u64 {
        u64::from(self.inner as u8)
    }

    fn __repr__(&self) -> String {
        format!("GameResult.{}", game_result_name(self.inner))
    }

    fn __str__(&self) -> &'static str {
        game_result_name(self.inner)
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        let other_value = match other.extract::<PyRef<'_, PyGameResult>>() {
            Ok(other) => Some(other.inner as u8),
            _ => match other.extract::<u8>() {
                Ok(value) => Some(value),
                _ => match other.extract::<u32>() {
                    Ok(value) => {
                        if value <= u32::from(u8::MAX) {
                            Some(value as u8)
                        } else {
                            None
                        }
                    }
                    _ => {
                        return Err(PyTypeError::new_err("unsupported comparison for GameResult"));
                    }
                },
            },
        };
        let current = self.inner as u8;
        match op {
            CompareOp::Eq => Ok(other_value == Some(current)),
            CompareOp::Ne => Ok(other_value != Some(current)),
            _ => Err(PyTypeError::new_err("GameResult only supports == and !=")),
        }
    }

    #[classmethod]
    fn from_str(_cls: &Bound<'_, PyType>, name: &str) -> PyResult<Self> {
        let inner = game_result_from_str(name)
            .ok_or_else(|| PyValueError::new_err(format!("invalid GameResult name: {name}")))?;
        Ok(Self { inner })
    }

    #[classattr]
    #[pyo3(name = "__members__")]
    fn members(py: Python<'_>) -> PyResult<Py<PyDict>> {
        let members = PyDict::new(py);
        for &name in GAME_RESULT_MEMBER_NAMES {
            let inner = game_result_from_str(name)
                .expect("GAME_RESULT_MEMBER_NAMES must contain valid GameResult names");
            members.set_item(name, Py::new(py, PyGameResult { inner })?)?;
        }
        Ok(members.unbind())
    }

    #[classattr]
    fn BLACK_WIN(py: Python<'_>) -> PyResult<Py<PyGameResult>> {
        Py::new(py, PyGameResult { inner: GameResult::BlackWin })
    }

    #[classattr]
    fn WHITE_WIN(py: Python<'_>) -> PyResult<Py<PyGameResult>> {
        Py::new(py, PyGameResult { inner: GameResult::WhiteWin })
    }

    #[classattr]
    fn DRAW_BY_REPETITION(py: Python<'_>) -> PyResult<Py<PyGameResult>> {
        Py::new(py, PyGameResult { inner: GameResult::DrawByRepetition })
    }

    #[classattr]
    fn ERROR(py: Python<'_>) -> PyResult<Py<PyGameResult>> {
        Py::new(py, PyGameResult { inner: GameResult::Error })
    }

    #[classattr]
    fn BLACK_WIN_BY_DECLARATION(py: Python<'_>) -> PyResult<Py<PyGameResult>> {
        Py::new(py, PyGameResult { inner: GameResult::BlackWinByDeclaration })
    }

    #[classattr]
    fn WHITE_WIN_BY_DECLARATION(py: Python<'_>) -> PyResult<Py<PyGameResult>> {
        Py::new(py, PyGameResult { inner: GameResult::WhiteWinByDeclaration })
    }

    #[classattr]
    fn DRAW_BY_MAX_PLIES(py: Python<'_>) -> PyResult<Py<PyGameResult>> {
        Py::new(py, PyGameResult { inner: GameResult::DrawByMaxPlies })
    }

    #[classattr]
    fn INVALID(py: Python<'_>) -> PyResult<Py<PyGameResult>> {
        Py::new(py, PyGameResult { inner: GameResult::Invalid })
    }

    #[classattr]
    fn BLACK_WIN_BY_FORFEIT(py: Python<'_>) -> PyResult<Py<PyGameResult>> {
        Py::new(py, PyGameResult { inner: GameResult::BlackWinByForfeit })
    }

    #[classattr]
    fn WHITE_WIN_BY_FORFEIT(py: Python<'_>) -> PyResult<Py<PyGameResult>> {
        Py::new(py, PyGameResult { inner: GameResult::WhiteWinByForfeit })
    }

    #[classattr]
    fn DRAW_BY_IMPASSE(py: Python<'_>) -> PyResult<Py<PyGameResult>> {
        Py::new(py, PyGameResult { inner: GameResult::DrawByImpasse })
    }

    #[classattr]
    fn BLACK_WIN_BY_TRY_RULE(py: Python<'_>) -> PyResult<Py<PyGameResult>> {
        Py::new(py, PyGameResult { inner: GameResult::BlackWinByTryRule })
    }

    #[classattr]
    fn WHITE_WIN_BY_TRY_RULE(py: Python<'_>) -> PyResult<Py<PyGameResult>> {
        Py::new(py, PyGameResult { inner: GameResult::WhiteWinByTryRule })
    }

    #[classattr]
    fn BLACK_WIN_BY_ILLEGAL_MOVE(py: Python<'_>) -> PyResult<Py<PyGameResult>> {
        Py::new(py, PyGameResult { inner: GameResult::BlackWinByIllegalMove })
    }

    #[classattr]
    fn WHITE_WIN_BY_ILLEGAL_MOVE(py: Python<'_>) -> PyResult<Py<PyGameResult>> {
        Py::new(py, PyGameResult { inner: GameResult::WhiteWinByIllegalMove })
    }

    #[classattr]
    fn PAUSED(py: Python<'_>) -> PyResult<Py<PyGameResult>> {
        Py::new(py, PyGameResult { inner: GameResult::Paused })
    }

    #[classattr]
    fn BLACK_WIN_BY_TIMEOUT(py: Python<'_>) -> PyResult<Py<PyGameResult>> {
        Py::new(py, PyGameResult { inner: GameResult::BlackWinByTimeout })
    }

    #[classattr]
    fn WHITE_WIN_BY_TIMEOUT(py: Python<'_>) -> PyResult<Py<PyGameResult>> {
        Py::new(py, PyGameResult { inner: GameResult::WhiteWinByTimeout })
    }
}

#[pyclass(name = "EngineInfo")]
#[derive(Clone)]
struct PyEngineInfo {
    inner: EngineInfo,
}

#[pymethods]
impl PyEngineInfo {
    #[new]
    #[pyo3(
        signature = (
            eval=None,
            depth=None,
            nodes=None,
            seldepth=None,
            wall_time_ms=None,
            latency_delta_ms=None,
            extras=None
        )
    )]
    fn new(
        eval: Option<i32>,
        depth: Option<u16>,
        nodes: Option<u64>,
        seldepth: Option<u16>,
        wall_time_ms: Option<i64>,
        latency_delta_ms: Option<i64>,
        extras: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let mut inner = EngineInfo::new()
            .with_eval(eval.map(Eval::from_i32))
            .with_depth(depth)
            .with_nodes(nodes)
            .with_seldepth(seldepth);
        if let Some(v) = wall_time_ms {
            inner.set_extra(Self::WALL_TIME_KEY.to_string(), EngineExtraValue::Int(v));
        }
        if let Some(v) = latency_delta_ms {
            inner.set_extra(Self::LATENCY_KEY.to_string(), EngineExtraValue::Int(v));
        }
        if let Some(extras) = extras {
            for (key, value) in parse_engine_extras_from_pyany(extras)? {
                inner.set_extra(key, value);
            }
        }
        Ok(Self { inner })
    }

    #[getter]
    fn eval(&self) -> Option<i32> {
        self.inner.eval().map(Eval::to_i32)
    }

    #[setter]
    fn set_eval(&mut self, eval: Option<i32>) {
        self.inner.set_eval(eval.map(Eval::from_i32));
    }

    #[getter]
    fn depth(&self) -> Option<u16> {
        self.inner.depth()
    }

    #[setter]
    fn set_depth(&mut self, depth: Option<u16>) {
        self.inner.set_depth(depth);
    }

    #[getter]
    fn nodes(&self) -> Option<u64> {
        self.inner.nodes()
    }

    #[setter]
    fn set_nodes(&mut self, nodes: Option<u64>) {
        self.inner.set_nodes(nodes);
    }

    #[getter]
    fn seldepth(&self) -> Option<u16> {
        self.inner.seldepth()
    }

    #[setter]
    fn set_seldepth(&mut self, seldepth: Option<u16>) {
        self.inner.set_seldepth(seldepth);
    }

    #[getter]
    fn wall_time_ms(&self) -> Option<i64> {
        self.inner
            .extras()
            .get(Self::WALL_TIME_KEY)
            .or_else(|| self.inner.extras().get(Self::WALL_TIME_KEY_ARENA))
            .and_then(|value| match value {
                EngineExtraValue::Int(v) => Some(*v),
                _ => None,
            })
    }

    #[setter]
    fn set_wall_time_ms(&mut self, value: Option<i64>) {
        let _ = self.inner.remove_extra(Self::WALL_TIME_KEY);
        let _ = self.inner.remove_extra(Self::WALL_TIME_KEY_ARENA);
        if let Some(v) = value {
            self.inner.set_extra(Self::WALL_TIME_KEY.to_string(), EngineExtraValue::Int(v));
        }
    }

    #[getter]
    fn latency_delta_ms(&self) -> Option<i64> {
        self.inner
            .extras()
            .get(Self::LATENCY_KEY)
            .or_else(|| self.inner.extras().get(Self::LATENCY_KEY_ARENA))
            .and_then(|value| match value {
                EngineExtraValue::Int(v) => Some(*v),
                _ => None,
            })
    }

    #[setter]
    fn set_latency_delta_ms(&mut self, value: Option<i64>) {
        let _ = self.inner.remove_extra(Self::LATENCY_KEY);
        let _ = self.inner.remove_extra(Self::LATENCY_KEY_ARENA);
        if let Some(v) = value {
            self.inner.set_extra(Self::LATENCY_KEY.to_string(), EngineExtraValue::Int(v));
        }
    }

    #[getter]
    fn extras<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        move_engine_extras_to_pydict(py, self.inner.extras())
    }

    #[setter]
    fn set_extras(&mut self, extras: &Bound<'_, PyAny>) -> PyResult<()> {
        self.inner.clear_extras();
        if extras.is_none() {
            return Ok(());
        }
        let extras = parse_engine_extras_from_pyany(extras)?;
        for (key, value) in extras {
            self.inner.set_extra(key, value);
        }
        Ok(())
    }

    fn set_extra(&mut self, key: String, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.inner.set_extra(key, move_engine_extra_from_pyany(value)?);
        Ok(())
    }

    fn remove_extra(&mut self, py: Python<'_>, key: &str) -> PyResult<Option<Py<PyAny>>> {
        self.inner
            .remove_extra(key)
            .map(|value| move_engine_extra_to_pyobject(py, &value))
            .transpose()
    }

    fn clear_extras(&mut self) {
        self.inner.clear_extras();
    }

    fn __repr__(&self) -> String {
        format!(
            "EngineInfo(eval={:?}, depth={:?}, nodes={:?}, seldepth={:?})",
            self.eval(),
            self.depth(),
            self.nodes(),
            self.seldepth()
        )
    }
}

impl PyEngineInfo {
    const WALL_TIME_KEY: &'static str = "wall_time_ms";
    const WALL_TIME_KEY_ARENA: &'static str = "arena.wall_time_ms";
    const LATENCY_KEY: &'static str = "latency_delta_ms";
    const LATENCY_KEY_ARENA: &'static str = "arena.latency_delta_ms";
}

#[pyclass(name = "MoveEntry")]
#[derive(Clone)]
struct PyMoveEntry {
    inner: MoveEntry,
    annotation: RecordAnnotation,
}

#[pymethods]
impl PyMoveEntry {
    #[new]
    #[pyo3(signature = (r#move, *, time_ms=None, comment=None, engine_info=None))]
    fn new(
        r#move: &Bound<'_, PyAny>,
        time_ms: Option<u32>,
        comment: Option<String>,
        engine_info: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let mv = parse_move_record_move(r#move)?;
        let mut annotation = RecordAnnotation::new().with_elapsed_ms(time_ms).with_comment(comment);
        if let Some(engine_info) = engine_info {
            annotation.set_engine_info(Some(move_engine_info_from_pyany(
                engine_info,
                ParseMode::Permissive,
            )?));
        }
        Ok(Self { inner: MoveEntry::new(mv), annotation })
    }

    #[getter]
    #[pyo3(name = "move")]
    fn move_(&self) -> PyMove {
        PyMove::from_move(self.inner.mv())
    }

    #[getter]
    fn comment(&self) -> Option<String> {
        self.annotation.comment().map(|value| value.to_string())
    }

    #[setter]
    fn set_comment(&mut self, comment: Option<String>) {
        self.annotation.set_comment(comment);
    }

    #[getter]
    fn time_ms(&self) -> Option<u32> {
        self.annotation.elapsed_ms()
    }

    #[setter]
    fn set_time_ms(&mut self, time_ms: Option<u32>) {
        self.annotation.set_elapsed_ms(time_ms);
    }

    #[getter]
    fn engine_info(&self) -> Option<PyEngineInfo> {
        self.annotation.engine_info().cloned().map(|inner| PyEngineInfo { inner })
    }

    #[setter]
    fn set_engine_info(&mut self, engine_info: Option<&PyEngineInfo>) {
        self.annotation.set_engine_info(engine_info.map(|value| value.inner.clone()));
    }

    fn __repr__(&self) -> String {
        format!(
            "MoveEntry(move={}, time_ms={:?}, has_engine_info={})",
            self.inner.mv().to_usi(),
            self.annotation.elapsed_ms(),
            self.annotation.engine_info().is_some()
        )
    }
}

#[pyclass(name = "SpecialMoveEntry")]
#[derive(Clone)]
struct PySpecialMoveEntry {
    inner: SpecialMoveEntry,
    annotation: RecordAnnotation,
}

#[pymethods]
impl PySpecialMoveEntry {
    #[new]
    #[pyo3(signature = (kind, result, unknown_name=None, comment=None, time_ms=None, raw=None))]
    fn new(
        kind: &str,
        result: &Bound<'_, PyAny>,
        unknown_name: Option<String>,
        comment: Option<String>,
        time_ms: Option<u32>,
        raw: Option<String>,
    ) -> PyResult<Self> {
        let result = parse_game_result_arg(result)?;
        let kind = special_move_from_name(kind, unknown_name)?;
        let annotation = RecordAnnotation::new().with_comment(comment).with_elapsed_ms(time_ms);
        let inner = SpecialMoveEntry::new(kind, result).with_raw(raw);
        Ok(Self { inner, annotation })
    }

    #[classmethod]
    #[pyo3(signature = (result, time_ms=None, comment=None, raw=None))]
    fn from_result(
        _cls: &Bound<'_, PyType>,
        result: &Bound<'_, PyAny>,
        time_ms: Option<u32>,
        comment: Option<String>,
        raw: Option<String>,
    ) -> PyResult<Self> {
        let result = parse_game_result_arg(result)?;
        let annotation = RecordAnnotation::new().with_elapsed_ms(time_ms).with_comment(comment);
        let inner =
            SpecialMoveEntry::new(special_move_from_game_result(result), result).with_raw(raw);
        Ok(Self { inner, annotation })
    }

    #[getter]
    fn kind(&self) -> &'static str {
        special_move_name(self.inner.kind())
    }

    #[getter]
    fn unknown_name(&self) -> Option<String> {
        match self.inner.kind() {
            SpecialMove::Unknown(name) => Some(name.clone()),
            _ => None,
        }
    }

    #[getter]
    fn result(&self) -> PyGameResult {
        PyGameResult { inner: self.inner.result() }
    }

    #[getter]
    fn comment(&self) -> Option<String> {
        self.annotation.comment().map(|value| value.to_string())
    }

    #[setter]
    fn set_comment(&mut self, comment: Option<String>) {
        self.annotation.set_comment(comment);
    }

    #[getter]
    fn time_ms(&self) -> Option<u32> {
        self.annotation.elapsed_ms()
    }

    #[setter]
    fn set_time_ms(&mut self, time_ms: Option<u32>) {
        self.annotation.set_elapsed_ms(time_ms);
    }

    #[getter]
    fn raw(&self) -> Option<String> {
        self.inner.raw().map(|value| value.to_string())
    }

    #[setter]
    fn set_raw(&mut self, raw: Option<String>) {
        self.inner.set_raw(raw);
    }

    fn __repr__(&self) -> String {
        format!(
            "SpecialMoveEntry(kind={}, result={}, time_ms={:?})",
            special_move_name(self.inner.kind()),
            game_result_name(self.inner.result()),
            self.annotation.elapsed_ms()
        )
    }
}

#[pyclass(name = "RecordEntry")]
#[derive(Clone)]
struct PyRecordEntry {
    inner: RecordEntry,
    annotation: RecordAnnotation,
}

#[pymethods]
impl PyRecordEntry {
    #[getter]
    fn kind(&self) -> &'static str {
        match &self.inner {
            RecordEntry::Move(_) => "move",
            RecordEntry::Special(_) => "special",
        }
    }

    #[getter]
    fn r#move(&self) -> Option<PyMoveEntry> {
        self.inner
            .as_move()
            .cloned()
            .map(|inner| PyMoveEntry { inner, annotation: self.annotation.clone() })
    }

    #[getter]
    fn special(&self) -> Option<PySpecialMoveEntry> {
        self.inner
            .as_special()
            .cloned()
            .map(|inner| PySpecialMoveEntry { inner, annotation: self.annotation.clone() })
    }

    fn __repr__(&self) -> String {
        match &self.inner {
            RecordEntry::Move(mv) => format!("RecordEntry(kind=move, move={})", mv.mv().to_usi()),
            RecordEntry::Special(special) => {
                format!("RecordEntry(kind=special, special={})", special_move_name(special.kind()))
            }
        }
    }
}

#[pyclass(name = "UsiPositionParts")]
#[derive(Clone)]
struct PyUsiPositionParts {
    initial_sfen: String,
    moves: Vec<Move>,
    final_sfen: String,
}

#[pymethods]
impl PyUsiPositionParts {
    #[getter]
    fn initial_sfen(&self) -> &str {
        &self.initial_sfen
    }

    #[getter]
    fn moves(&self) -> Vec<PyMove> {
        self.moves.iter().copied().map(PyMove::from_move).collect()
    }

    #[getter]
    fn move_usi(&self) -> Vec<String> {
        self.moves.iter().map(|mv| mv.to_usi()).collect()
    }

    #[getter]
    fn final_sfen(&self) -> &str {
        &self.final_sfen
    }

    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let out = PyDict::new(py);
        out.set_item("initial_sfen", &self.initial_sfen)?;
        out.set_item("moves", self.moves())?;
        out.set_item("move_usi", self.move_usi())?;
        out.set_item("final_sfen", &self.final_sfen)?;
        Ok(out)
    }

    fn __repr__(&self) -> String {
        format!(
            "UsiPositionParts(initial_sfen={:?}, moves={}, final_sfen={:?})",
            self.initial_sfen,
            self.moves.len(),
            self.final_sfen
        )
    }
}

#[pyclass(name = "RecordNodeId")]
#[derive(Clone, Copy)]
struct PyRecordNodeId {
    inner: RecordNodeId,
}

#[pymethods]
impl PyRecordNodeId {
    #[new]
    fn new(raw: usize) -> PyResult<Self> {
        let inner = RecordNodeId::try_from_raw(raw)
            .ok_or_else(|| PyIndexError::new_err("node_id out of range"))?;
        Ok(Self { inner })
    }

    #[getter]
    fn raw(&self) -> usize {
        self.inner.raw()
    }

    fn __int__(&self) -> usize {
        self.inner.raw()
    }

    fn __index__(&self) -> usize {
        self.inner.raw()
    }

    fn __repr__(&self) -> String {
        format!("RecordNodeId({})", self.inner.raw())
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        let other = if let Ok(other) = other.extract::<PyRef<'_, Self>>() {
            other.inner.raw()
        } else if let Ok(value) = other.extract::<usize>() {
            value
        } else {
            return Ok(matches!(op, CompareOp::Ne));
        };
        let equal = self.inner.raw() == other;
        Ok(match op {
            CompareOp::Eq => equal,
            CompareOp::Ne => !equal,
            _ => false,
        })
    }
}

#[pyclass(name = "RecordMetadataKey")]
struct PyRecordMetadataKey;

#[pymethods]
impl PyRecordMetadataKey {
    #[new]
    fn new() -> PyResult<Self> {
        Err(PyTypeError::new_err("RecordMetadataKey is a constants namespace"))
    }

    #[classattr]
    #[pyo3(name = "EVENT")]
    const EVENT: &'static str = "event";

    #[classattr]
    #[pyo3(name = "SITE")]
    const SITE: &'static str = "site";

    #[classattr]
    #[pyo3(name = "BLACK_PLAYER")]
    const BLACK_PLAYER: &'static str = "black_player";

    #[classattr]
    #[pyo3(name = "WHITE_PLAYER")]
    const WHITE_PLAYER: &'static str = "white_player";

    #[classattr]
    #[pyo3(name = "GAME_NAME")]
    const GAME_NAME: &'static str = "game_name";

    #[classattr]
    #[pyo3(name = "GAME_TYPE")]
    const GAME_TYPE: &'static str = "game_type";

    #[classattr]
    #[pyo3(name = "START_DATE")]
    const START_DATE: &'static str = "start_date";

    #[classattr]
    #[pyo3(name = "END_DATE")]
    const END_DATE: &'static str = "end_date";

    #[classattr]
    #[pyo3(name = "UPDATED_DATE")]
    const UPDATED_DATE: &'static str = "updated_date";

    #[classattr]
    #[pyo3(name = "COMMENT")]
    const COMMENT: &'static str = "comment";
}

#[pyclass(name = "RecordMetadata")]
#[derive(Clone)]
struct PyRecordMetadata {
    inner: RecordMetadata,
}

#[pymethods]
impl PyRecordMetadata {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(
        signature = (
            event=None,
            site=None,
            black_player=None,
            white_player=None,
            game_name=None,
            game_type=None,
            time_control=None,
            black_time_control=None,
            white_time_control=None,
            max_moves=None,
            impasse_rule=None,
            start_date=None,
            end_date=None,
            updated_date=None,
            comment=None,
            attributes=None
        )
    )]
    fn new(
        event: Option<String>,
        site: Option<String>,
        black_player: Option<String>,
        white_player: Option<String>,
        game_name: Option<String>,
        game_type: Option<String>,
        time_control: Option<&PyTimeControl>,
        black_time_control: Option<&PyTimeControl>,
        white_time_control: Option<&PyTimeControl>,
        max_moves: Option<u32>,
        impasse_rule: Option<String>,
        start_date: Option<String>,
        end_date: Option<String>,
        updated_date: Option<String>,
        comment: Option<String>,
        attributes: Option<HashMap<String, String>>,
    ) -> Self {
        let mut builder = RecordMetadata::builder();
        builder
            .event(event)
            .site(site)
            .black_player(black_player)
            .white_player(white_player)
            .game_name(game_name)
            .game_type(game_type)
            .time_control(time_control.map(|tc| tc.inner))
            .black_time_control(black_time_control.map(|tc| tc.inner))
            .white_time_control(white_time_control.map(|tc| tc.inner))
            .max_moves(max_moves)
            .impasse_rule(impasse_rule)
            .start_date(start_date)
            .end_date(end_date)
            .updated_date(updated_date)
            .comment(comment);
        if let Some(attributes) = attributes {
            for (key, value) in attributes {
                builder.add_attribute(key, value);
            }
        }
        Self { inner: builder.build() }
    }

    #[getter]
    fn event(&self) -> Option<String> {
        self.inner.event().map(|value| value.to_string())
    }

    #[setter]
    fn set_event(&mut self, value: Option<String>) {
        let mut builder = self.to_builder();
        builder.event(value);
        self.inner = builder.build();
    }

    #[getter]
    fn site(&self) -> Option<String> {
        self.inner.site().map(|value| value.to_string())
    }

    #[setter]
    fn set_site(&mut self, value: Option<String>) {
        let mut builder = self.to_builder();
        builder.site(value);
        self.inner = builder.build();
    }

    #[getter]
    fn black_player(&self) -> Option<String> {
        self.inner.black_player().map(|value| value.to_string())
    }

    #[setter]
    fn set_black_player(&mut self, value: Option<String>) {
        let mut builder = self.to_builder();
        builder.black_player(value);
        self.inner = builder.build();
    }

    #[getter]
    fn white_player(&self) -> Option<String> {
        self.inner.white_player().map(|value| value.to_string())
    }

    #[setter]
    fn set_white_player(&mut self, value: Option<String>) {
        let mut builder = self.to_builder();
        builder.white_player(value);
        self.inner = builder.build();
    }

    #[getter]
    fn game_name(&self) -> Option<String> {
        self.inner.game_name().map(ToOwned::to_owned)
    }

    #[setter]
    fn set_game_name(&mut self, value: Option<String>) {
        let mut builder = self.to_builder();
        builder.game_name(value);
        self.inner = builder.build();
    }

    #[getter]
    fn game_type(&self) -> Option<String> {
        self.inner.game_type().map(ToOwned::to_owned)
    }

    #[setter]
    fn set_game_type(&mut self, value: Option<String>) {
        let mut builder = self.to_builder();
        builder.game_type(value);
        self.inner = builder.build();
    }

    #[getter]
    fn time_control(&self) -> Option<PyTimeControl> {
        self.inner.time_control().cloned().map(|value| PyTimeControl { inner: value })
    }

    #[setter]
    fn set_time_control(&mut self, value: Option<&PyTimeControl>) {
        let mut builder = self.to_builder();
        builder.time_control(value.map(|tc| tc.inner));
        self.inner = builder.build();
    }

    #[getter]
    #[pyo3(name = "black_time_control")]
    fn black_time_control(&self) -> Option<PyTimeControl> {
        self.inner.black_time_control().cloned().map(|value| PyTimeControl { inner: value })
    }

    #[setter]
    #[pyo3(name = "black_time_control")]
    fn set_black_time_control(&mut self, value: Option<&PyTimeControl>) {
        let mut builder = self.to_builder();
        builder.black_time_control(value.map(|tc| tc.inner));
        self.inner = builder.build();
    }

    #[getter]
    #[pyo3(name = "white_time_control")]
    fn white_time_control(&self) -> Option<PyTimeControl> {
        self.inner.white_time_control().cloned().map(|value| PyTimeControl { inner: value })
    }

    #[setter]
    #[pyo3(name = "white_time_control")]
    fn set_white_time_control(&mut self, value: Option<&PyTimeControl>) {
        let mut builder = self.to_builder();
        builder.white_time_control(value.map(|tc| tc.inner));
        self.inner = builder.build();
    }

    #[getter]
    fn max_moves(&self) -> Option<u32> {
        self.inner.max_moves()
    }

    #[setter]
    fn set_max_moves(&mut self, value: Option<u32>) {
        let mut builder = self.to_builder();
        builder.max_moves(value);
        self.inner = builder.build();
    }

    #[getter]
    fn impasse_rule(&self) -> Option<String> {
        self.inner.impasse_rule().map(|value| value.to_string())
    }

    #[setter]
    fn set_impasse_rule(&mut self, value: Option<String>) {
        let mut builder = self.to_builder();
        builder.impasse_rule(value);
        self.inner = builder.build();
    }

    #[getter]
    fn start_date(&self) -> Option<String> {
        self.inner.start_date().map(|value| value.to_string())
    }

    #[setter]
    fn set_start_date(&mut self, value: Option<String>) {
        let mut builder = self.to_builder();
        builder.start_date(value);
        self.inner = builder.build();
    }

    #[getter]
    fn end_date(&self) -> Option<String> {
        self.inner.end_date().map(|value| value.to_string())
    }

    #[setter]
    fn set_end_date(&mut self, value: Option<String>) {
        let mut builder = self.to_builder();
        builder.end_date(value);
        self.inner = builder.build();
    }

    #[getter]
    fn updated_date(&self) -> Option<String> {
        self.inner.updated_date().map(ToOwned::to_owned)
    }

    #[setter]
    fn set_updated_date(&mut self, value: Option<String>) {
        let mut builder = self.to_builder();
        builder.updated_date(value);
        self.inner = builder.build();
    }

    #[getter]
    fn comment(&self) -> Option<String> {
        self.inner.comment().map(|value| value.to_string())
    }

    #[setter]
    fn set_comment(&mut self, value: Option<String>) {
        let mut builder = self.to_builder();
        builder.comment(value);
        self.inner = builder.build();
    }

    #[getter]
    fn attributes(&self) -> HashMap<String, String> {
        self.inner.attributes().iter().map(|(key, value)| (key.clone(), value.clone())).collect()
    }

    #[setter]
    fn set_attributes(&mut self, attributes: HashMap<String, String>) {
        let mut builder = self.to_builder_without_attributes();
        for (key, value) in attributes {
            builder.add_attribute(key, value);
        }
        self.inner = builder.build();
    }

    fn set_attribute(&mut self, key: String, value: String) {
        let mut attributes = self.attributes();
        attributes.insert(key, value);
        self.set_attributes(attributes);
    }

    fn remove_attribute(&mut self, key: &str) -> Option<String> {
        let mut attributes = self.attributes();
        let removed = attributes.remove(key);
        self.set_attributes(attributes);
        removed
    }

    fn clear_attributes(&mut self) {
        self.set_attributes(HashMap::new());
    }

    fn __repr__(&self) -> String {
        format!(
            "RecordMetadata(event={:?}, site={:?}, players={:?}/{:?})",
            self.inner.event(),
            self.inner.site(),
            self.inner.black_player(),
            self.inner.white_player()
        )
    }
}

impl PyRecordMetadata {
    fn to_builder(&self) -> ::rsshogi::records::record::RecordMetadataBuilder {
        let mut builder = self.to_builder_without_attributes();
        for (key, value) in self.inner.attributes() {
            builder.add_attribute(key.clone(), value.clone());
        }
        builder
    }

    fn to_builder_without_attributes(&self) -> ::rsshogi::records::record::RecordMetadataBuilder {
        let mut builder = RecordMetadata::builder();
        builder
            .event(self.inner.event().map(ToOwned::to_owned))
            .site(self.inner.site().map(ToOwned::to_owned))
            .black_player(self.inner.black_player().map(ToOwned::to_owned))
            .white_player(self.inner.white_player().map(ToOwned::to_owned))
            .game_name(self.inner.game_name().map(ToOwned::to_owned))
            .game_type(self.inner.game_type().map(ToOwned::to_owned))
            .time_control(self.inner.time_control().copied())
            .black_time_control(self.inner.black_time_control().copied())
            .white_time_control(self.inner.white_time_control().copied())
            .max_moves(self.inner.max_moves())
            .impasse_rule(self.inner.impasse_rule().map(ToOwned::to_owned))
            .start_date(self.inner.start_date().map(ToOwned::to_owned))
            .end_date(self.inner.end_date().map(ToOwned::to_owned))
            .updated_date(self.inner.updated_date().map(ToOwned::to_owned))
            .comment(self.inner.comment().map(ToOwned::to_owned));
        builder
    }
}

#[pyclass(name = "Record")]
#[derive(Clone)]
pub(crate) struct PyRecord {
    pub(crate) inner: Record,
}

#[pymethods]
impl PyRecord {
    #[new]
    #[pyo3(signature = (init_position_sfen, moves=None, metadata=None, terminal=None, initial_comment=None))]
    fn new(
        init_position_sfen: &str,
        moves: Option<Vec<PyMoveEntry>>,
        metadata: Option<&PyRecordMetadata>,
        terminal: Option<&PySpecialMoveEntry>,
        initial_comment: Option<String>,
    ) -> PyResult<Self> {
        let mut inner = record_from_py_main_line(
            init_position_sfen.to_string(),
            moves.unwrap_or_default(),
            terminal,
        )?;
        if let Some(metadata) = metadata {
            inner.set_metadata(metadata.inner.clone());
        }
        inner.set_initial_comment(initial_comment);
        Ok(Self { inner })
    }

    #[getter]
    fn init_position_sfen(&self) -> &str {
        self.inner.init_position_sfen()
    }

    #[getter]
    fn initial_comment(&self) -> Option<String> {
        self.inner.initial_comment().map(ToOwned::to_owned)
    }

    #[setter(initial_comment)]
    fn set_initial_comment_prop(&mut self, initial_comment: Option<String>) {
        self.inner.set_initial_comment(initial_comment);
    }

    #[getter]
    fn moves(&self) -> Vec<PyMoveEntry> {
        self.inner
            .main_line_ids()
            .into_iter()
            .filter_map(|node_id| {
                let node = self.inner.node(node_id);
                node.mv()
                    .cloned()
                    .map(|inner| PyMoveEntry { inner, annotation: node.annotation().clone() })
            })
            .collect()
    }

    #[getter]
    fn metadata(&self) -> PyRecordMetadata {
        PyRecordMetadata { inner: self.inner.metadata().clone() }
    }

    #[getter]
    fn game_name(&self) -> Option<String> {
        self.inner.metadata().game_name().map(ToOwned::to_owned)
    }

    #[getter]
    fn game_type(&self) -> Option<String> {
        self.inner.metadata().game_type().map(ToOwned::to_owned)
    }

    #[getter]
    fn updated_date(&self) -> Option<String> {
        self.inner.metadata().updated_date().map(ToOwned::to_owned)
    }

    #[getter]
    fn black_time_control(&self) -> Option<PyTimeControl> {
        self.inner.metadata().black_time_control().copied().map(|inner| PyTimeControl { inner })
    }

    #[getter]
    fn white_time_control(&self) -> Option<PyTimeControl> {
        self.inner.metadata().white_time_control().copied().map(|inner| PyTimeControl { inner })
    }

    #[setter(metadata)]
    fn set_metadata_prop(&mut self, metadata: &PyRecordMetadata) {
        self.inner.set_metadata(metadata.inner.clone());
    }

    #[getter]
    fn result(&self) -> PyGameResult {
        PyGameResult { inner: self.inner.result() }
    }

    #[getter]
    fn result_info(&self) -> PyGameResultInfo {
        game_result_info_from_record(&self.inner)
    }

    #[getter]
    fn end_time_ms(&self) -> Option<u32> {
        self.inner.main_terminal_node().and_then(|node_id| self.inner.node(node_id).time_ms())
    }

    #[getter]
    fn end_comment(&self) -> Option<String> {
        self.inner
            .main_terminal_node()
            .and_then(|node_id| self.inner.node(node_id).comment())
            .map(ToOwned::to_owned)
    }

    #[getter]
    fn main_terminal(&self) -> Option<PySpecialMoveEntry> {
        self.inner.main_terminal_node().and_then(|node_id| {
            let node = self.inner.node(node_id);
            node.special()
                .cloned()
                .map(|inner| PySpecialMoveEntry { inner, annotation: node.annotation().clone() })
        })
    }

    #[setter(main_terminal)]
    fn set_main_terminal_prop(&mut self, terminal: Option<&PySpecialMoveEntry>) -> PyResult<()> {
        if let Some(terminal) = terminal {
            self.inner
                .set_main_terminal_with_annotation(
                    terminal.inner.clone(),
                    terminal.annotation.clone(),
                )
                .map_err(|e| PyValueError::new_err(e.to_string()))?;
            Ok(())
        } else {
            Err(PyValueError::new_err("main_terminal cannot be cleared (set a SpecialMoveEntry)"))
        }
    }

    #[getter]
    fn move_count(&self) -> usize {
        self.inner.move_count()
    }

    #[pyo3(signature = (patch, *, strict=false))]
    fn update_metadata(&mut self, patch: &Bound<'_, PyAny>, strict: bool) -> PyResult<()> {
        if let Ok(metadata) = patch.extract::<PyRef<'_, PyRecordMetadata>>() {
            self.inner.set_metadata(metadata.inner.clone());
            return Ok(());
        }
        let patch_dict = patch
            .cast::<PyDict>()
            .map_err(|_| PyTypeError::new_err("update_metadata expects RecordMetadata or dict"))?;
        let updated = apply_metadata_patch(
            self.inner.metadata(),
            patch_dict,
            ParseMode::from_strict(strict),
        )?;
        self.inner.set_metadata(updated);
        Ok(())
    }

    fn set_metadata_attribute(&mut self, key: String, value: String) {
        let mut metadata = PyRecordMetadata { inner: self.inner.metadata().clone() };
        metadata.set_attribute(key, value);
        self.inner.set_metadata(metadata.inner);
    }

    fn remove_metadata_attribute(&mut self, key: &str) -> Option<String> {
        let mut metadata = PyRecordMetadata { inner: self.inner.metadata().clone() };
        let removed = metadata.remove_attribute(key);
        self.inner.set_metadata(metadata.inner);
        removed
    }

    fn clear_metadata_attributes(&mut self) {
        let mut metadata = PyRecordMetadata { inner: self.inner.metadata().clone() };
        metadata.clear_attributes();
        self.inner.set_metadata(metadata.inner);
    }

    fn extend_main_line(&mut self, moves: Vec<PyMoveEntry>) -> PyResult<Vec<usize>> {
        let tail = self.inner.main_line_tail();
        append_py_moves_to_record(&mut self.inner, tail, moves)
    }

    fn set_main_terminal(&mut self, terminal: &PySpecialMoveEntry) -> PyResult<usize> {
        Ok(self
            .inner
            .set_main_terminal_with_annotation(terminal.inner.clone(), terminal.annotation.clone())
            .map_err(|e| PyValueError::new_err(e.to_string()))?
            .raw())
    }

    #[classmethod]
    fn from_kif_str(_cls: &Bound<'_, PyType>, text: &str) -> PyResult<Self> {
        let record =
            parse_kif_str_record(text).map_err(|err| PyValueError::new_err(err.to_string()))?;
        Ok(Self { inner: record })
    }

    #[classmethod]
    fn from_ki2_str(_cls: &Bound<'_, PyType>, text: &str) -> PyResult<Self> {
        let record =
            parse_ki2_str_record(text).map_err(|err| PyValueError::new_err(err.to_string()))?;
        Ok(Self { inner: record })
    }

    #[classmethod]
    fn from_csa_str(_cls: &Bound<'_, PyType>, text: &str) -> PyResult<Self> {
        let record =
            parse_csa_str_record(text).map_err(|err| PyValueError::new_err(err.to_string()))?;
        Ok(Self { inner: record })
    }

    #[classmethod]
    fn from_jkf_str(_cls: &Bound<'_, PyType>, text: &str) -> PyResult<Self> {
        let record =
            parse_jkf_str_record(text).map_err(|err| PyValueError::new_err(err.to_string()))?;
        Ok(Self { inner: record })
    }

    #[classmethod]
    #[pyo3(signature = (position, *, allow_special_tokens=false))]
    fn from_usi_position(
        _cls: &Bound<'_, PyType>,
        position: &str,
        allow_special_tokens: bool,
    ) -> PyResult<Self> {
        let options = UsiPositionParseOptions { allow_special_tokens };
        let record = parse_usi_position_record(position, options)
            .map_err(|err| PyValueError::new_err(err.to_string()))?;
        Ok(Self { inner: record })
    }

    #[classmethod]
    fn from_sbinpack(_cls: &Bound<'_, PyType>, data: &Bound<'_, PyAny>) -> PyResult<Self> {
        let bytes: Vec<u8> = data.extract()?;
        let chain = single_sbinpack_chain_from_bytes(&bytes)?;
        let record = sbinpack_chain_to_record(&chain)
            .map_err(|err| PyValueError::new_err(format!("sbinpack: {err}")))?;
        Ok(Self { inner: record })
    }

    #[classmethod]
    fn from_sbinpack_with_metadata<'py>(
        _cls: &Bound<'_, PyType>,
        py: Python<'py>,
        data: &Bound<'_, PyAny>,
    ) -> PyResult<(Self, Bound<'py, PyBytes>)> {
        let bytes: Vec<u8> = data.extract()?;
        let chain = single_sbinpack_chain_from_bytes(&bytes)?;
        let metadata = PyBytes::new(py, chain.metadata.as_bytes());
        let record = sbinpack_chain_to_record(&chain)
            .map_err(|err| PyValueError::new_err(format!("sbinpack: {err}")))?;
        Ok((Self { inner: record }, metadata))
    }

    #[classmethod]
    fn from_pack(_cls: &Bound<'_, PyType>, data: &Bound<'_, PyAny>) -> PyResult<Self> {
        let bytes: Vec<u8> = data.extract()?;
        let mut records = records_from_bytes_pack(&bytes)
            .map_err(|err| PyValueError::new_err(format!("pack: {err}")))?;
        if records.is_empty() {
            return Err(PyValueError::new_err("pack: no games found"));
        }
        if records.len() > 1 {
            return Err(PyValueError::new_err("pack: multiple games not supported"));
        }
        Ok(Self { inner: records.remove(0) })
    }

    #[classmethod]
    #[pyo3(signature = (init_position_sfen, moves, terminal=None, metadata=None, initial_comment=None))]
    fn from_main_line(
        _cls: &Bound<'_, PyType>,
        init_position_sfen: &str,
        moves: Vec<PyMoveEntry>,
        terminal: Option<&PySpecialMoveEntry>,
        metadata: Option<&PyRecordMetadata>,
        initial_comment: Option<String>,
    ) -> PyResult<Self> {
        let mut inner = record_from_py_main_line(init_position_sfen.to_string(), moves, terminal)?;
        if let Some(metadata) = metadata {
            inner.set_metadata(metadata.inner.clone());
        }
        inner.set_initial_comment(initial_comment);
        Ok(Self { inner })
    }

    #[classmethod]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        init_position_sfen,
        usi_moves,
        result=None,
        move_times_ms=None,
        evals=None,
        nodes=None,
        depths=None,
        seldepths=None,
        wall_times_ms=None,
        latency_deltas_ms=None,
        metadata=None,
        initial_comment=None
    ))]
    fn from_usi_main_line(
        _cls: &Bound<'_, PyType>,
        init_position_sfen: &str,
        usi_moves: Vec<String>,
        result: Option<&Bound<'_, PyAny>>,
        move_times_ms: Option<Vec<Option<u32>>>,
        evals: Option<Vec<Option<i32>>>,
        nodes: Option<Vec<Option<u64>>>,
        depths: Option<Vec<Option<u16>>>,
        seldepths: Option<Vec<Option<u16>>>,
        wall_times_ms: Option<Vec<Option<i64>>>,
        latency_deltas_ms: Option<Vec<Option<i64>>>,
        metadata: Option<&PyRecordMetadata>,
        initial_comment: Option<String>,
    ) -> PyResult<Self> {
        let parsed_initial = parse_usi_position_parts_inner(init_position_sfen)?;
        if !parsed_initial.parts.moves.is_empty() {
            return Err(PyValueError::new_err(
                "init_position_sfen must not include moves; pass moves via usi_moves",
            ));
        }
        let init_position_sfen = parsed_initial.parts.final_sfen;
        let mut pos = parsed_initial.position;

        let mut move_records = Vec::with_capacity(usi_moves.len());
        for (index, usi) in usi_moves.iter().enumerate() {
            let mv16 = Move::from_usi(usi).ok_or_else(|| {
                PyValueError::new_err(format!("invalid USI move at index {index}: {usi}"))
            })?;
            let mv32 = pos.move32_from_move(mv16);
            if !mv32.is_normal() {
                return Err(PyValueError::new_err(format!(
                    "invalid move value at index {index}: {usi}"
                )));
            }
            if !pos.is_legal_move32(mv32) {
                return Err(PyValueError::new_err(format!("illegal move at index {index}: {usi}")));
            }

            let mut engine_info = EngineInfo::new()
                .with_eval(optional_index(&evals, index).map(Eval::from_i32))
                .with_nodes(optional_index(&nodes, index))
                .with_depth(optional_index(&depths, index))
                .with_seldepth(optional_index(&seldepths, index));
            if let Some(value) = optional_index(&wall_times_ms, index) {
                engine_info.set_extra(
                    PyEngineInfo::WALL_TIME_KEY.to_string(),
                    EngineExtraValue::Int(value),
                );
            }
            if let Some(value) = optional_index(&latency_deltas_ms, index) {
                engine_info
                    .set_extra(PyEngineInfo::LATENCY_KEY.to_string(), EngineExtraValue::Int(value));
            }
            let engine_info = if engine_info.eval().is_some()
                || engine_info.nodes().is_some()
                || engine_info.depth().is_some()
                || engine_info.seldepth().is_some()
                || !engine_info.extras().is_empty()
            {
                Some(engine_info)
            } else {
                None
            };

            move_records.push(PyMoveEntry {
                inner: MoveEntry::new(mv16),
                annotation: RecordAnnotation::new()
                    .with_elapsed_ms(optional_index(&move_times_ms, index))
                    .with_engine_info(engine_info),
            });
            pos.apply_move32(mv32);
        }

        let terminal =
            result.map(parse_game_result_arg).transpose()?.map(|result| PySpecialMoveEntry {
                inner: SpecialMoveEntry::new(special_move_from_game_result(result), result),
                annotation: RecordAnnotation::new(),
            });
        let mut inner =
            record_from_py_main_line(init_position_sfen, move_records, terminal.as_ref())?;
        if let Some(metadata) = metadata {
            inner.set_metadata(metadata.inner.clone());
        }
        inner.set_initial_comment(initial_comment);
        Ok(Self { inner })
    }

    fn to_kif(&self) -> PyResult<String> {
        export_kif_record(&self.inner).map_err(|err| PyValueError::new_err(err.to_string()))
    }

    fn to_ki2(&self) -> PyResult<String> {
        export_ki2_record(&self.inner).map_err(|err| PyValueError::new_err(err.to_string()))
    }

    fn to_csa(&self) -> PyResult<String> {
        export_csa_record(&self.inner).map_err(|err| PyValueError::new_err(err.to_string()))
    }

    fn to_jkf(&self) -> PyResult<String> {
        export_jkf_record(&self.inner).map_err(|err| PyValueError::new_err(err.to_string()))
    }

    #[pyo3(signature = (*, include_special_tokens=false))]
    fn to_usi_position(&self, include_special_tokens: bool) -> PyResult<String> {
        export_usi_position_record(&self.inner, UsiPositionExportOptions { include_special_tokens })
            .map_err(|err| PyValueError::new_err(err.to_string()))
    }

    #[pyo3(signature = (*, include_main=true, include_variations=false))]
    fn to_psv<'py>(
        &self,
        py: Python<'py>,
        include_main: bool,
        include_variations: bool,
    ) -> PyResult<Bound<'py, PyList>> {
        let mut position = Position::empty();
        position
            .set_sfen(self.inner.init_position_sfen())
            .map_err(|err| PyValueError::new_err(err.to_string()))?;

        let out = PyList::empty(py);
        if !include_main && !include_variations {
            return Ok(out);
        }
        let options = PsvCollectOptions {
            game_result: self.inner.result(),
            include_main,
            include_variations,
        };
        collect_record_psv_entries(
            &self.inner,
            self.inner.root_id(),
            &position,
            true,
            &options,
            &out,
        )?;
        Ok(out)
    }

    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let out = PyDict::new(py);
        out.set_item("version", 1)?;
        out.set_item("init_position_sfen", self.inner.init_position_sfen())?;
        out.set_item("initial_comment", self.inner.initial_comment())?;

        let moves = PyList::empty(py);
        for node_id in self.inner.main_line_ids() {
            let node = self.inner.node(node_id);
            if let Some(mv) = node.mv() {
                moves.append(move_record_to_pydict(py, mv, node.annotation())?)?;
            }
        }
        out.set_item("moves", moves)?;
        out.set_item("metadata", metadata_to_pydict(py, self.inner.metadata())?)?;
        let result_info = game_result_info_from_record(&self.inner);
        out.set_item("result", game_result_info_to_pydict(py, &result_info)?)?;

        Ok(out)
    }

    #[classmethod]
    #[pyo3(signature = (data, *, strict=false))]
    fn from_dict(
        _cls: &Bound<'_, PyType>,
        data: &Bound<'_, PyAny>,
        strict: bool,
    ) -> PyResult<Self> {
        let dict =
            data.cast::<PyDict>().map_err(|_| PyTypeError::new_err("from_dict expects dict"))?;
        let mode = ParseMode::from_strict(strict);

        if mode.is_strict() {
            for (key_any, _) in dict.iter() {
                let key = key_any
                    .extract::<String>()
                    .map_err(|_| PyTypeError::new_err("from_dict: top-level keys must be str"))?;
                if key != "version"
                    && key != "init_position_sfen"
                    && key != "initial_comment"
                    && key != "moves"
                    && key != "metadata"
                    && key != "result"
                {
                    return Err(PyValueError::new_err(format!(
                        "from_dict: unknown top-level key '{key}'"
                    )));
                }
            }
        }

        let init_position_sfen = dict_required_string(dict, "init_position_sfen")?;
        let moves = match dict.get_item("moves")? {
            Some(moves_any) => {
                let parsed = move_records_from_sequence(&moves_any, mode)?;
                normalize_record_moves(&init_position_sfen, parsed)?
            }
            _ => Vec::new(),
        };
        if dict.get_item("main_terminal")?.is_some() {
            return Err(PyValueError::new_err(
                "from_dict: 'main_terminal' is no longer supported; use 'result' payload",
            ));
        }

        let terminal = dict
            .get_item("result")?
            .map(|value| parse_result_payload(&value, mode))
            .transpose()?
            .and_then(|(result_value, reason, end_time_ms, end_comment)| {
                terminal_from_result_payload(result_value, reason, end_time_ms, end_comment)
            });

        let mut inner = record_from_py_main_line(init_position_sfen, moves, terminal.as_ref())?;
        if let Some(initial_comment_any) = dict.get_item("initial_comment")?
            && !initial_comment_any.is_none()
        {
            let initial_comment = initial_comment_any.extract::<String>().map_err(|_| {
                PyTypeError::new_err("from_dict: 'initial_comment' must be str or None")
            })?;
            inner.set_initial_comment(Some(initial_comment));
        }

        if let Some(metadata_any) = dict.get_item("metadata")?
            && !metadata_any.is_none()
        {
            let metadata = metadata_from_pyany(&metadata_any, mode)?;
            inner.set_metadata(metadata);
        }

        Ok(Self { inner })
    }

    #[classmethod]
    #[pyo3(signature = (path, encoding=None))]
    fn from_kif_file(
        _cls: &Bound<'_, PyType>,
        path: &Bound<'_, PyAny>,
        encoding: Option<&str>,
    ) -> PyResult<Self> {
        let text = read_text_with_encoding(path, encoding, default_kif_encoding)?;
        let record =
            parse_kif_str_record(&text).map_err(|err| PyValueError::new_err(err.to_string()))?;
        Ok(Self { inner: record })
    }

    #[classmethod]
    #[pyo3(signature = (path, encoding="shift_jis"))]
    fn from_ki2_file(
        _cls: &Bound<'_, PyType>,
        path: &Bound<'_, PyAny>,
        encoding: Option<&str>,
    ) -> PyResult<Self> {
        let text = read_text_with_encoding(path, encoding, default_ki2_encoding)?;
        let record =
            parse_ki2_str_record(&text).map_err(|err| PyValueError::new_err(err.to_string()))?;
        Ok(Self { inner: record })
    }

    #[classmethod]
    #[pyo3(signature = (path, encoding="shift_jis"))]
    fn from_csa_file(
        _cls: &Bound<'_, PyType>,
        path: &Bound<'_, PyAny>,
        encoding: Option<&str>,
    ) -> PyResult<Self> {
        let text = read_text_with_encoding(path, encoding, default_csa_encoding)?;
        let record =
            parse_csa_str_record(&text).map_err(|err| PyValueError::new_err(err.to_string()))?;
        Ok(Self { inner: record })
    }

    #[classmethod]
    #[pyo3(signature = (path, encoding="utf-8"))]
    fn from_jkf_file(
        _cls: &Bound<'_, PyType>,
        path: &Bound<'_, PyAny>,
        encoding: Option<&str>,
    ) -> PyResult<Self> {
        let text = read_text_with_encoding(path, encoding, default_jkf_encoding)?;
        let record =
            parse_jkf_str_record(&text).map_err(|err| PyValueError::new_err(err.to_string()))?;
        Ok(Self { inner: record })
    }

    #[pyo3(signature = (path, encoding=None))]
    fn write_kif(&self, path: &Bound<'_, PyAny>, encoding: Option<&str>) -> PyResult<()> {
        let text = self.to_kif()?;
        write_text_with_encoding(path, &text, encoding, default_kif_encoding)
    }

    #[pyo3(signature = (path, encoding="shift_jis"))]
    fn write_csa(&self, path: &Bound<'_, PyAny>, encoding: Option<&str>) -> PyResult<()> {
        let text = self.to_csa()?;
        write_text_with_encoding(path, &text, encoding, default_csa_encoding)
    }

    #[pyo3(signature = (path, encoding="shift_jis"))]
    fn write_ki2(&self, path: &Bound<'_, PyAny>, encoding: Option<&str>) -> PyResult<()> {
        let text = self.to_ki2()?;
        write_text_with_encoding(path, &text, encoding, default_ki2_encoding)
    }

    #[pyo3(signature = (path, encoding="utf-8"))]
    fn write_jkf(&self, path: &Bound<'_, PyAny>, encoding: Option<&str>) -> PyResult<()> {
        let text = self.to_jkf()?;
        write_text_with_encoding(path, &text, encoding, default_jkf_encoding)
    }

    #[pyo3(signature = (
        stem_score=0,
        include_main=true,
        include_variations=false,
        metadata=None
    ))]
    fn to_sbinpack<'py>(
        &self,
        py: Python<'py>,
        stem_score: i16,
        include_main: bool,
        include_variations: bool,
        metadata: Option<Vec<u8>>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let metadata = SbinpackMetadata::new(metadata.unwrap_or_default())
            .map_err(|err| PyValueError::new_err(format!("sbinpack: {err:?}")))?;
        let chains = chains_from_record_sbinpack(
            &self.inner,
            stem_score,
            metadata,
            include_main,
            include_variations,
        )
        .map_err(|err| PyValueError::new_err(format!("sbinpack: {err:?}")))?;
        if chains.is_empty() {
            return Err(PyValueError::new_err("sbinpack: no chains to serialize"));
        }
        let bytes = serialize_sbinpack_file(&chains)
            .map_err(|err| PyValueError::new_err(format!("sbinpack: {err:?}")))?;
        Ok(PyBytes::new(py, &bytes))
    }

    fn to_pack<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let game = game_from_record_pack(&self.inner)
            .map_err(|err| PyValueError::new_err(format!("pack: {err}")))?;
        let bytes = encode_game_pack(&game);
        Ok(PyBytes::new(py, &bytes))
    }

    fn root_node_id(&self) -> usize {
        self.inner.root_id().raw()
    }

    fn main_line_ids(&self) -> Vec<usize> {
        self.inner.main_line_ids().iter().map(|id| id.raw()).collect()
    }

    fn node_parent(&self, node_id: usize) -> PyResult<Option<usize>> {
        let node_id = self.node_id_or_err(node_id)?;
        Ok(self.inner.node(node_id).parent().map(|id| id.raw()))
    }

    fn node_move(&self, node_id: usize) -> PyResult<Option<PyMoveEntry>> {
        let node_id = self.node_id_or_err(node_id)?;
        let node = self.inner.node(node_id);
        Ok(node
            .mv()
            .cloned()
            .map(|inner| PyMoveEntry { inner, annotation: node.annotation().clone() }))
    }

    fn node_entry(&self, node_id: usize) -> PyResult<Option<PyRecordEntry>> {
        let node_id = self.node_id_or_err(node_id)?;
        let node = self.inner.node(node_id);
        Ok(node
            .entry()
            .cloned()
            .map(|inner| PyRecordEntry { inner, annotation: node.annotation().clone() }))
    }

    fn node_terminal(&self, node_id: usize) -> PyResult<Option<PySpecialMoveEntry>> {
        let node_id = self.node_id_or_err(node_id)?;
        let node = self.inner.node(node_id);
        Ok(node
            .special()
            .cloned()
            .map(|inner| PySpecialMoveEntry { inner, annotation: node.annotation().clone() }))
    }

    fn children(&self, node_id: usize) -> PyResult<Vec<usize>> {
        let node_id = self.node_id_or_err(node_id)?;
        Ok(self.inner.children(node_id).iter().map(|id| id.raw()).collect())
    }

    #[pyo3(name = "into_editor")]
    fn to_editor(&self) -> PyResult<PyRecordEditor> {
        let editor = RecordEditor::from_record(self.inner.clone())
            .map_err(|err| PyValueError::new_err(err.to_string()))?;
        Ok(PyRecordEditor { inner: Some(Box::new(editor)) })
    }

    fn __repr__(&self) -> String {
        format!(
            "Record(init_position_sfen='{}', moves={}, result={})",
            self.inner.init_position_sfen(),
            self.inner.move_count(),
            game_result_name(self.inner.result())
        )
    }
}

#[pyclass(name = "RecordEditor")]
struct PyRecordEditor {
    inner: Option<Box<RecordEditor>>,
}

#[pymethods]
impl PyRecordEditor {
    #[new]
    fn new(init_position: &str) -> PyResult<Self> {
        let init_position = if init_position.trim() == "startpos" {
            RecordInitialPosition::Startpos
        } else {
            RecordInitialPosition::Sfen(init_position.to_string())
        };
        let editor = RecordEditor::new(init_position)
            .map_err(|err| PyValueError::new_err(err.to_string()))?;
        Ok(Self { inner: Some(Box::new(editor)) })
    }

    #[classmethod]
    fn from_record(_cls: &Bound<'_, PyType>, record: &PyRecord) -> PyResult<Self> {
        record.to_editor()
    }

    #[getter]
    fn current_node(&self) -> PyResult<usize> {
        Ok(self.editor()?.current_node().raw())
    }

    #[getter]
    fn position(&self) -> PyResult<PyBoard> {
        Ok(PyBoard { position: Box::new(self.editor()?.position().clone()) })
    }

    #[getter]
    fn position_sfen(&self) -> PyResult<String> {
        Ok(self.editor()?.position().to_sfen(None))
    }

    fn record(&self) -> PyResult<PyRecord> {
        Ok(PyRecord { inner: self.editor()?.record().clone() })
    }

    #[pyo3(name = "into_record")]
    fn take_record(&mut self) -> PyResult<PyRecord> {
        let editor = self
            .inner
            .take()
            .ok_or_else(|| PyValueError::new_err("RecordEditor has already been consumed"))?;
        Ok(PyRecord { inner: (*editor).into_record() })
    }

    fn go_to(&mut self, node_id: usize) -> PyResult<()> {
        let id = RecordNodeId::try_from_raw(node_id)
            .ok_or_else(|| PyIndexError::new_err("node_id out of range"))?;
        self.editor_mut()?.go_to(id).map_err(|err| PyValueError::new_err(err.to_string()))
    }

    fn go_back(&mut self) -> PyResult<bool> {
        self.editor_mut()?.go_back().map_err(|err| PyValueError::new_err(err.to_string()))
    }

    #[pyo3(signature = (child_index=0))]
    fn go_forward(&mut self, child_index: usize) -> PyResult<bool> {
        self.editor_mut()?
            .go_forward(child_index)
            .map_err(|err| PyValueError::new_err(err.to_string()))
    }

    #[pyo3(signature = (child_index=0))]
    fn branch_to(&mut self, child_index: usize) -> PyResult<bool> {
        self.editor_mut()?
            .branch_to(child_index)
            .map_err(|err| PyValueError::new_err(err.to_string()))
    }

    fn append_move(&mut self, r#move: &PyMoveEntry) -> PyResult<usize> {
        Ok(self
            .editor_mut()?
            .append_move_with_annotation(r#move.inner.clone(), r#move.annotation.clone())
            .map_err(|err| PyValueError::new_err(err.to_string()))?
            .raw())
    }

    fn append_special(&mut self, special: &PySpecialMoveEntry) -> PyResult<usize> {
        Ok(self
            .editor_mut()?
            .append_special_with_annotation(special.inner.clone(), special.annotation.clone())
            .map_err(|err| PyValueError::new_err(err.to_string()))?
            .raw())
    }

    fn __repr__(&self) -> String {
        match self.inner.as_ref() {
            Some(editor) => format!("RecordEditor(current_node={})", editor.current_node().raw()),
            None => "RecordEditor(consumed)".to_string(),
        }
    }
}

impl PyRecordEditor {
    fn editor(&self) -> PyResult<&RecordEditor> {
        self.inner
            .as_ref()
            .map(Box::as_ref)
            .ok_or_else(|| PyValueError::new_err("RecordEditor has already been consumed"))
    }

    fn editor_mut(&mut self) -> PyResult<&mut RecordEditor> {
        self.inner
            .as_mut()
            .map(Box::as_mut)
            .ok_or_else(|| PyValueError::new_err("RecordEditor has already been consumed"))
    }
}

impl PyRecord {
    fn node_id_or_err(&self, node_id: usize) -> PyResult<RecordNodeId> {
        let id = RecordNodeId::try_from_raw(node_id)
            .ok_or_else(|| PyIndexError::new_err("node_id out of range"))?;
        self.inner.try_node(id).map_err(|_| PyIndexError::new_err("node_id out of range"))?;
        Ok(id)
    }
}

fn record_from_py_main_line(
    init_position_sfen: String,
    moves: Vec<PyMoveEntry>,
    terminal: Option<&PySpecialMoveEntry>,
) -> PyResult<Record> {
    let mut record =
        Record::new(init_position_sfen).map_err(|err| PyValueError::new_err(err.to_string()))?;
    let root_id = record.root_id();
    append_py_moves_to_record(&mut record, root_id, moves)?;
    if let Some(terminal) = terminal {
        record
            .set_main_terminal_with_annotation(terminal.inner.clone(), terminal.annotation.clone())
            .map_err(|err| PyValueError::new_err(err.to_string()))?;
    }
    Ok(record)
}

fn append_py_moves_to_record(
    record: &mut Record,
    mut parent: RecordNodeId,
    moves: Vec<PyMoveEntry>,
) -> PyResult<Vec<usize>> {
    let mut ids = Vec::with_capacity(moves.len());
    for mv in moves {
        let child = record
            .append_move_with_annotation(parent, mv.inner, mv.annotation)
            .map_err(|err| PyValueError::new_err(err.to_string()))?;
        ids.push(child.raw());
        parent = child;
    }
    Ok(ids)
}

struct PsvCollectOptions {
    game_result: GameResult,
    include_main: bool,
    include_variations: bool,
}

fn collect_record_psv_entries(
    record: &Record,
    node_id: RecordNodeId,
    position: &Position,
    on_main_line: bool,
    options: &PsvCollectOptions,
    out: &Bound<'_, PyList>,
) -> PyResult<()> {
    let py = out.py();
    for (index, &child_id) in record.children(node_id).iter().enumerate() {
        let child_on_main_line = on_main_line && index == 0;
        let include_node = (child_on_main_line && options.include_main)
            || (!child_on_main_line && options.include_variations);
        let node = record.node(child_id);
        if let Some(mv_record) = node.mv() {
            if include_node {
                let eval = node.eval().ok_or_else(|| {
                    PyValueError::new_err(format!(
                        "to_psv: missing eval at node_id={} (move={})",
                        child_id.raw(),
                        mv_record.mv().to_usi()
                    ))
                })?;
                let value =
                    build_psv_entry(position, eval.raw(), mv_record.mv(), options.game_result);
                out.append(PyBytes::new(py, &value))?;
            }

            let mut next = position.clone();
            next.apply_move(mv_record.mv());
            collect_record_psv_entries(record, child_id, &next, child_on_main_line, options, out)?;
        } else {
            collect_record_psv_entries(
                record,
                child_id,
                position,
                child_on_main_line,
                options,
                out,
            )?;
        }
    }
    Ok(())
}

fn build_psv_entry(position: &Position, score: i16, mv: Move, game_result: GameResult) -> [u8; 40] {
    let packed = position.to_packed_sfen();
    let mut value = [0u8; 40];
    value[..32].copy_from_slice(&packed.data);
    value[32..34].copy_from_slice(&score.to_le_bytes());
    value[34..36].copy_from_slice(&mv.raw().to_le_bytes());
    value[36..38].copy_from_slice(&position.game_ply().to_le_bytes());
    value[38] = (game_result as i8) as u8;
    value[39] = 0;
    value
}

pub(crate) fn path_to_string(path: &Bound<'_, PyAny>) -> PyResult<String> {
    let os = path.py().import("os")?;
    let fspath = os.getattr("fspath")?.call1((path,))?;
    fspath.extract::<String>()
}

fn default_kif_encoding(path: &Bound<'_, PyAny>) -> PyResult<&'static str> {
    let path_str = path_to_string(path)?;
    let ext = Path::new(&path_str)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    Ok(if ext == "kifu" { "utf-8" } else { "shift_jis" })
}

fn default_csa_encoding(_path: &Bound<'_, PyAny>) -> PyResult<&'static str> {
    Ok("shift_jis")
}

fn default_ki2_encoding(_path: &Bound<'_, PyAny>) -> PyResult<&'static str> {
    Ok("shift_jis")
}

fn default_jkf_encoding(_path: &Bound<'_, PyAny>) -> PyResult<&'static str> {
    Ok("utf-8")
}

fn read_text_with_encoding(
    path: &Bound<'_, PyAny>,
    encoding: Option<&str>,
    default_encoding: fn(&Bound<'_, PyAny>) -> PyResult<&'static str>,
) -> PyResult<String> {
    let py = path.py();
    let pathlib = py.import("pathlib")?;
    let path_obj = pathlib.getattr("Path")?.call1((path,))?;
    let encoding = match encoding {
        Some(value) => value,
        None => default_encoding(path)?,
    };
    let kwargs = PyDict::new(py);
    kwargs.set_item("encoding", encoding)?;
    let text = path_obj.call_method("read_text", (), Some(&kwargs))?;
    text.extract::<String>()
}

fn write_text_with_encoding(
    path: &Bound<'_, PyAny>,
    text: &str,
    encoding: Option<&str>,
    default_encoding: fn(&Bound<'_, PyAny>) -> PyResult<&'static str>,
) -> PyResult<()> {
    let py = path.py();
    let pathlib = py.import("pathlib")?;
    let path_obj = pathlib.getattr("Path")?.call1((path,))?;
    let encoding = match encoding {
        Some(value) => value,
        None => default_encoding(path)?,
    };
    let kwargs = PyDict::new(py);
    kwargs.set_item("encoding", encoding)?;
    path_obj.call_method("write_text", (text,), Some(&kwargs))?;
    Ok(())
}

#[pyclass(name = "Svg")]
#[derive(Clone)]
struct PySvg {
    inner: String,
}

#[pymethods]
impl PySvg {
    fn _repr_svg_(&self) -> &str {
        &self.inner
    }

    fn __str__(&self) -> &str {
        &self.inner
    }

    fn __repr__(&self) -> &str {
        &self.inner
    }
}

#[pyclass(name = "ValidationIssue")]
#[derive(Clone)]
struct PyValidationIssue {
    inner: RawValidationIssue,
}

#[pymethods]
impl PyValidationIssue {
    #[getter]
    fn kind(&self) -> &'static str {
        match self.inner {
            RawValidationIssue::NoKing(_) => "NO_KING",
            RawValidationIssue::TwoKings(_) => "TWO_KINGS",
            RawValidationIssue::DoublePawn(_, _) => "DOUBLE_PAWN",
            RawValidationIssue::InvalidHandCount { .. } => "INVALID_HAND_COUNT",
            RawValidationIssue::InvalidPlacement(_, _) => "INVALID_PLACEMENT",
        }
    }

    #[getter]
    fn color(&self) -> Option<PyColor> {
        match self.inner {
            RawValidationIssue::NoKing(color)
            | RawValidationIssue::TwoKings(color)
            | RawValidationIssue::DoublePawn(_, color) => Some(PyColor::from_color(color)),
            RawValidationIssue::InvalidHandCount { .. }
            | RawValidationIssue::InvalidPlacement(_, _) => None,
        }
    }

    #[getter]
    fn file_usi(&self) -> Option<String> {
        match self.inner {
            RawValidationIssue::DoublePawn(file, _) => Some(file.to_usi().to_string()),
            _ => None,
        }
    }

    #[getter]
    fn square(&self) -> Option<PySquare> {
        match self.inner {
            RawValidationIssue::InvalidPlacement(square, _) => Some(PySquare::from_square(square)),
            _ => None,
        }
    }

    #[getter]
    fn piece_type(&self) -> Option<PyPieceType> {
        match self.inner {
            RawValidationIssue::InvalidHandCount { piece, .. } => {
                Some(PyPieceType::from_piece_type(piece.to_piece_type()))
            }
            RawValidationIssue::InvalidPlacement(_, piece_type) => {
                Some(PyPieceType::from_piece_type(piece_type))
            }
            _ => None,
        }
    }

    #[getter]
    fn count(&self) -> Option<u32> {
        match self.inner {
            RawValidationIssue::InvalidHandCount { count, .. } => Some(count),
            _ => None,
        }
    }

    #[getter]
    fn message(&self) -> String {
        self.inner.to_string()
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!("ValidationIssue(kind='{}', message={:?})", self.kind(), self.inner.to_string())
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        let other_value =
            other.extract::<PyRef<'_, PyValidationIssue>>().ok().map(|other| other.inner.clone());
        compare_eq(op, other_value.map(|other| self.inner == other), "ValidationIssue")
    }
}

impl PyValidationIssue {
    fn from_issue(inner: RawValidationIssue) -> Self {
        Self { inner }
    }
}

#[pyclass(name = "ValidationReport")]
#[derive(Clone)]
struct PyValidationReport {
    inner: RawValidationReport,
}

#[pymethods]
impl PyValidationReport {
    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    #[getter]
    fn issues(&self) -> Vec<PyValidationIssue> {
        self.inner.issues().iter().cloned().map(PyValidationIssue::from_issue).collect()
    }

    fn __len__(&self) -> usize {
        self.inner.issues().len()
    }

    fn __repr__(&self) -> String {
        format!(
            "ValidationReport(is_valid={}, issue_count={})",
            self.inner.is_valid(),
            self.inner.issues().len()
        )
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        let other_value =
            other.extract::<PyRef<'_, PyValidationReport>>().ok().map(|other| other.inner.clone());
        compare_eq(op, other_value.map(|other| self.inner == other), "ValidationReport")
    }
}

impl PyValidationReport {
    fn from_report(inner: RawValidationReport) -> Self {
        Self { inner }
    }
}

#[pyclass(name = "PositionState")]
#[derive(Clone)]
struct PyPositionState {
    inner: RawPositionState,
}

#[pymethods]
impl PyPositionState {
    #[new]
    #[pyo3(signature = (sfen=None))]
    fn new(sfen: Option<&str>) -> PyResult<Self> {
        let inner = if let Some(sfen) = sfen {
            board::parse_sfen(sfen).map_err(|err| PyValueError::new_err(err.to_string()))?
        } else {
            RawPositionState {
                board: BoardArray::empty(),
                hands: [Hand::ZERO; Color::COUNT],
                side_to_move: Color::BLACK,
                ply: 1,
            }
        };
        Ok(Self { inner })
    }

    #[classmethod]
    fn from_sfen(_cls: &Bound<'_, PyType>, sfen: &str) -> PyResult<Self> {
        Self::new(Some(sfen))
    }

    fn copy(&self) -> Self {
        self.clone()
    }

    fn __copy__(&self) -> Self {
        self.copy()
    }

    #[pyo3(signature = (include_ply=true))]
    fn to_sfen(&self, include_ply: bool) -> String {
        board::generate_sfen_from_position_state(&self.inner, include_ply)
    }

    fn piece_on(&self, square: &PySquare) -> PyResult<PyPiece> {
        ensure_on_board_square(square.inner())?;
        Ok(PyPiece::from_piece(self.inner.board.get(square.inner())))
    }

    fn set_piece(&mut self, square: &PySquare, piece: &PyPiece) -> PyResult<()> {
        ensure_on_board_square(square.inner())?;
        self.inner.board.set(square.inner(), piece.inner());
        Ok(())
    }

    fn hand(&self, color: &PyColor) -> PyHand {
        PyHand::from_hand(self.inner.hands[color.inner().to_index()])
    }

    fn set_hand(&mut self, color: &PyColor, hand: &PyHand) {
        self.inner.hands[color.inner().to_index()] = hand.inner();
    }

    fn validate(&self) -> PyResult<()> {
        raw_position_from_position_state(&self.inner)?
            .validate()
            .map_err(|err| PyValueError::new_err(validation_error_message(&err)))
    }

    fn is_valid(&self) -> PyResult<bool> {
        Ok(raw_position_from_position_state(&self.inner)?.is_valid())
    }

    fn validate_all(&self) -> PyResult<PyValidationReport> {
        Ok(PyValidationReport::from_report(
            raw_position_from_position_state(&self.inner)?.validate_all(),
        ))
    }

    #[getter]
    fn side_to_move(&self) -> PyColor {
        PyColor::from_color(self.inner.side_to_move)
    }

    #[setter]
    fn set_side_to_move(&mut self, color: &PyColor) {
        self.inner.side_to_move = color.inner();
    }

    #[getter]
    fn ply(&self) -> u32 {
        u32::from(self.inner.ply)
    }

    #[setter]
    fn set_ply(&mut self, ply: u32) -> PyResult<()> {
        self.inner.ply = u16::try_from(ply)
            .map_err(|_| PyValueError::new_err("ply must fit in uint16 range (0..65535)"))?;
        Ok(())
    }

    #[getter]
    fn board(&self) -> Vec<PyPiece> {
        self.inner.board.iter().map(|(_, piece)| PyPiece::from_piece(piece)).collect()
    }

    #[getter]
    fn hands(&self) -> (PyHand, PyHand) {
        (
            PyHand::from_hand(self.inner.hands[Color::BLACK.to_index()]),
            PyHand::from_hand(self.inner.hands[Color::WHITE.to_index()]),
        )
    }

    fn __repr__(&self) -> String {
        format!(
            "PositionState(side_to_move={}, ply={}, sfen={:?})",
            self.inner.side_to_move,
            self.inner.ply,
            board::generate_sfen_from_position_state(&self.inner, true)
        )
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        let other_value =
            other.extract::<PyRef<'_, PyPositionState>>().ok().map(|other| other.inner);
        compare_eq(op, other_value.map(|other| self.inner == other), "PositionState")
    }
}

impl PyPositionState {
    fn from_position_state(inner: RawPositionState) -> Self {
        Self { inner }
    }
}

#[pyclass(unsendable, name = "Board")]
struct PyBoard {
    // `Position` は `#[repr(align(64))]` のため、PyO3 が確保する PyObject 領域に
    // そのまま埋め込むとアラインメント違反で abort する。
    // ヒープに逃がして正しいアラインメントを保証する。
    position: Box<Position>,
}

#[pymethods]
impl PyBoard {
    #[new]
    #[pyo3(signature = (sfen=None))]
    fn new(sfen: Option<&str>) -> PyResult<Self> {
        board::init();
        let mut position = Box::new(Position::empty());
        if let Some(sfen) = sfen {
            position.set_sfen(sfen).map_err(|err| PyValueError::new_err(err.to_string()))?;
        } else {
            position.set_hirate();
        }
        position.init_stack();
        Ok(Self { position })
    }

    fn to_sfen(&self) -> String {
        self.position.to_sfen(None)
    }

    fn to_position_state(&self) -> PyPositionState {
        PyPositionState::from_position_state(board::generate_position_state(&self.position))
    }

    fn copy(&self) -> Self {
        Self { position: self.position.clone() }
    }

    fn __copy__(&self) -> Self {
        self.copy()
    }

    #[pyo3(signature = (out=None))]
    fn to_packed_sfen(
        &self,
        py: Python<'_>,
        out: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let packed = self.position.to_packed_sfen();
        if let Some(out) = out {
            fill_psfen_output(out, &packed.data)?;
            Ok(py.None())
        } else {
            Ok(PyBytes::new(py, &packed.data).unbind().into())
        }
    }

    #[pyo3(signature = (out=None))]
    fn to_hcp(&self, py: Python<'_>, out: Option<&Bound<'_, PyAny>>) -> PyResult<Py<PyAny>> {
        let packed = self.position.to_huffman_coded_pos();
        if let Some(out) = out {
            fill_hcp_output(out, &packed.data)?;
            Ok(py.None())
        } else {
            Ok(PyBytes::new(py, &packed.data).unbind().into())
        }
    }

    #[pyo3(signature = (mv=None, score=0, game_result=None, game_ply=None, out=None))]
    fn to_psv(
        &self,
        py: Python<'_>,
        mv: Option<&Bound<'_, PyAny>>,
        score: i16,
        game_result: Option<&Bound<'_, PyAny>>,
        game_ply: Option<u32>,
        out: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let packed = self.position.to_packed_sfen();
        let mv = mv.map_or(Ok(Move::MOVE_NONE), parse_move_arg)?;
        let result = game_result.map_or(Ok(GameResult::Invalid), parse_game_result_arg)?;

        let ply_u32 = game_ply.unwrap_or(self.position.game_ply() as u32);
        let ply_u16 = u16::try_from(ply_u32)
            .map_err(|_| PyValueError::new_err("game_ply must fit in uint16 range (0..65535)"))?;

        let mut value = [0u8; 40];
        value[..32].copy_from_slice(&packed.data);
        value[32..34].copy_from_slice(&score.to_le_bytes());
        value[34..36].copy_from_slice(&mv.raw().to_le_bytes());
        value[36..38].copy_from_slice(&ply_u16.to_le_bytes());
        value[38] = (result as i8) as u8;
        value[39] = 0;

        if let Some(out) = out {
            fill_psfen_value_output(out, &value, score, mv.raw(), ply_u16, result as i8)?;
            Ok(py.None())
        } else {
            Ok(PyBytes::new(py, &value).unbind().into())
        }
    }

    fn set_sfen(&mut self, sfen: &str) -> PyResult<()> {
        self.position.set_sfen(sfen).map_err(|err| PyValueError::new_err(err.to_string()))?;
        self.position.init_stack();
        Ok(())
    }

    fn set_position_state(&mut self, state: &PyPositionState) {
        self.position.set_position_state(&state.inner);
        self.position.init_stack();
    }

    fn set_usi_position(&mut self, position: &str) -> PyResult<()> {
        *self.position = parse_usi_position_inner(position)?;
        Ok(())
    }

    fn set_packed_sfen(&mut self, psfen: &Bound<'_, PyAny>) -> PyResult<()> {
        let data = extract_psfen_bytes(psfen)?;
        let packed = PackedSfen { data };
        self.position
            .set_packed_sfen(&packed, false, 1)
            .map_err(|err| PyValueError::new_err(format!("{err:?}")))?;
        self.position.init_stack();
        Ok(())
    }

    #[pyo3(signature = (hcp, game_ply=1))]
    fn set_hcp(&mut self, hcp: &Bound<'_, PyAny>, game_ply: u32) -> PyResult<()> {
        let ply = u16::try_from(game_ply)
            .map_err(|_| PyValueError::new_err("game_ply must fit in uint16 range (0..65535)"))?;
        let data = extract_hcp_bytes(hcp)?;
        let packed = HuffmanCodedPos { data };
        self.position
            .set_huffman_coded_pos(&packed, ply)
            .map_err(|err| PyValueError::new_err(err.to_string()))?;
        self.position.init_stack();
        Ok(())
    }

    #[pyo3(signature = (best_move=None, score=0, game_result=None, out=None))]
    fn to_hcpe(
        &self,
        py: Python<'_>,
        best_move: Option<&Bound<'_, PyAny>>,
        score: i16,
        game_result: Option<&Bound<'_, PyAny>>,
        out: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let best_move = best_move.map_or(Ok(AperyMove::from_raw(0)), parse_apery_move_arg)?;
        let result = game_result.map_or_else(
            || Err(PyValueError::new_err("game_result is required for hcpe export")),
            parse_hcpe_game_result_arg,
        )?;
        let entry = HuffmanCodedPosAndEval::new(
            self.position.to_huffman_coded_pos(),
            score,
            best_move,
            result,
        );
        let value = encode_hcpe_entry(&entry);

        if let Some(out) = out {
            fill_hcpe_output(out, &value, score, best_move.raw(), result as i8)?;
            Ok(py.None())
        } else {
            Ok(PyBytes::new(py, &value).unbind().into())
        }
    }

    fn zobrist_hash(&self) -> u128 {
        let key = self.position.key();
        (u128::from(key.high_u64()) << 64) | u128::from(key.low_u64())
    }

    fn can_declare_win(&self) -> bool {
        self.position.declaration_win_move() != MOVE_NONE
    }

    fn evaluate_declaration<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        declaration_evaluation_to_pydict(py, self.position.evaluate_declaration())
    }

    fn move32_from_move(&self, mv: &PyMove) -> PyResult<PyMove32> {
        let mv = self.position.move32_from_move(mv.inner);
        if !mv.is_normal() {
            return Err(PyValueError::new_err("invalid move value"));
        }
        Ok(PyMove32 { inner: mv })
    }

    fn apery_move32_from_move(&self, mv: &PyMove) -> PyAperyMove32 {
        PyAperyMove32 { inner: self.position.apery_move32_from_move(mv.inner) }
    }

    fn apery_move32_from_move32(&self, mv: &PyMove32) -> PyAperyMove32 {
        PyAperyMove32 { inner: self.position.apery_move32_from_move32(mv.inner) }
    }

    fn move_from_csa(&self, csa: &str) -> PyResult<PyMove32> {
        let mv = self.position.move_from_csa(csa);
        if !mv.is_normal() {
            return Err(PyValueError::new_err("invalid CSA move"));
        }
        Ok(PyMove32 { inner: mv })
    }

    fn apply_move(&mut self, r#move: PyRef<'_, PyMove>) -> PyResult<()> {
        let move_value = self.position.move32_from_move(r#move.inner);
        if !self.position.is_legal_move32(move_value) {
            return Err(PyValueError::new_err("illegal move"));
        }
        self.position.apply_move32(move_value);
        Ok(())
    }

    fn apply_move32(&mut self, r#move: PyRef<'_, PyMove32>) -> PyResult<()> {
        let move_value = r#move.inner;
        if !move_value.is_normal() {
            return Err(PyValueError::new_err("invalid move value"));
        }
        if !self.position.is_legal_move32(move_value) {
            return Err(PyValueError::new_err("illegal move"));
        }
        self.position.apply_move32(move_value);
        Ok(())
    }

    fn push_move(&mut self, r#move: PyRef<'_, PyMove>) -> PyResult<PyMove32> {
        let move_value = self.position.move32_from_move(r#move.inner);
        if !self.position.is_legal_move32(move_value) {
            return Err(PyValueError::new_err("illegal move"));
        }
        self.position.apply_move32(move_value);
        Ok(PyMove32 { inner: move_value })
    }

    fn push_move32(&mut self, r#move: PyRef<'_, PyMove32>) -> PyResult<PyMove32> {
        let move_value = r#move.inner;
        if !move_value.is_normal() {
            return Err(PyValueError::new_err("invalid move value"));
        }
        if !self.position.is_legal_move32(move_value) {
            return Err(PyValueError::new_err("illegal move"));
        }
        self.position.apply_move32(move_value);
        Ok(PyMove32 { inner: move_value })
    }

    fn apply_usi(&mut self, usi: &str) -> PyResult<()> {
        let mv16 = Move::from_usi(usi).ok_or_else(|| PyValueError::new_err("invalid USI move"))?;
        let move_value = self.position.move32_from_move(mv16);
        if !move_value.is_normal() {
            return Err(PyValueError::new_err("invalid move value"));
        }
        if !self.position.is_legal_move32(move_value) {
            return Err(PyValueError::new_err("illegal move"));
        }
        self.position.apply_move32(move_value);
        Ok(())
    }

    fn push_usi(&mut self, usi: &str) -> PyResult<PyMove32> {
        let mv16 = Move::from_usi(usi).ok_or_else(|| PyValueError::new_err("invalid USI move"))?;
        let move_value = self.position.move32_from_move(mv16);
        if !move_value.is_normal() {
            return Err(PyValueError::new_err("invalid move value"));
        }
        if !self.position.is_legal_move32(move_value) {
            return Err(PyValueError::new_err("illegal move"));
        }
        self.position.apply_move32(move_value);
        Ok(PyMove32 { inner: move_value })
    }

    fn push_usi_with_delta<'py>(
        &mut self,
        py: Python<'py>,
        usi: &str,
    ) -> PyResult<Bound<'py, PyDict>> {
        let mv16 = Move::from_usi(usi).ok_or_else(|| PyValueError::new_err("invalid USI move"))?;
        let move_value = self.position.move32_from_move(mv16);
        if !move_value.is_normal() {
            return Err(PyValueError::new_err("invalid move value"));
        }
        if !self.position.is_legal_move32(move_value) {
            return Err(PyValueError::new_err("illegal move"));
        }
        let gives_check = self.position.gives_check_move32(move_value);
        let delta = self.position.apply_move32_with_delta(move_value, gives_check);
        move_delta_to_pydict(py, move_value, delta)
    }

    fn solve_mate_in_one(&self) -> Option<PyMove32> {
        mate::solve_mate_in_one(&self.position).map(|mv| PyMove32 { inner: mv })
    }

    fn apply_csa(&mut self, csa: &str) -> PyResult<()> {
        let move_value = self.position.move_from_csa(csa);
        if !move_value.is_normal() {
            return Err(PyValueError::new_err("invalid CSA move"));
        }
        if !self.position.is_legal_move32(move_value) {
            return Err(PyValueError::new_err("illegal move"));
        }
        self.position.apply_move32(move_value);
        Ok(())
    }

    fn undo_move(&mut self, r#move: PyRef<'_, PyMove>) -> PyResult<()> {
        let mv16 = r#move.inner;
        let last = self.position.last_move();
        if !last.is_normal() || last.to_move() != mv16 {
            return Err(PyValueError::new_err("can only undo the last move"));
        }
        self.position.undo_move32(last).map_err(|err| PyValueError::new_err(format!("{err:?}")))?;
        Ok(())
    }

    fn undo_move32(&mut self, r#move: PyRef<'_, PyMove32>) -> PyResult<()> {
        let move_value = r#move.inner;
        let last = self.position.last_move();
        if !last.is_normal() || last != move_value {
            return Err(PyValueError::new_err("can only undo the last move"));
        }
        self.position
            .undo_move32(move_value)
            .map_err(|err| PyValueError::new_err(format!("{err:?}")))?;
        Ok(())
    }

    fn pop(&mut self) -> PyResult<Option<PyMove32>> {
        let last = self.position.last_move();
        if !last.is_normal() {
            return Ok(None);
        }
        self.position.undo_move32(last).map_err(|err| PyValueError::new_err(format!("{err:?}")))?;
        Ok(Some(PyMove32 { inner: last }))
    }

    fn last_move(&self) -> Option<PyMove32> {
        let mv = self.position.last_move();
        if mv.is_normal() { Some(PyMove32 { inner: mv }) } else { None }
    }

    fn piece_on(&self, square: &PySquare) -> PyPiece {
        PyPiece::from_piece(self.position.piece_on(square.inner()))
    }

    fn is_square_empty(&self, square: &PySquare) -> bool {
        self.position.is_square_empty(square.inner())
    }

    fn empties(&self) -> PyBitboard {
        PyBitboard::from_bitboard(self.position.empties())
    }

    fn hand(&self, color: &PyColor) -> PyHand {
        PyHand::from_hand(self.position.hand(color.inner()))
    }

    fn hand_counts(&self, py: Python<'_>, color: &PyColor) -> PyResult<Py<PyDict>> {
        let hand = self.position.hand(color.inner());
        let counts = PyDict::new(py);
        for (key, hand_piece) in [
            ("P", HandPiece::PAWN),
            ("L", HandPiece::LANCE),
            ("N", HandPiece::KNIGHT),
            ("S", HandPiece::SILVER),
            ("B", HandPiece::BISHOP),
            ("R", HandPiece::ROOK),
            ("G", HandPiece::GOLD),
        ] {
            counts.set_item(key, hand.count(hand_piece))?;
        }
        Ok(counts.unbind())
    }

    fn king_square(&self, color: &PyColor) -> PySquare {
        PySquare::from_square(self.position.king_square(color.inner()))
    }

    #[getter]
    fn turn(&self) -> PyColor {
        PyColor::from_color(self.position.turn())
    }

    #[getter]
    fn game_ply(&self) -> u32 {
        self.position.game_ply() as u32
    }

    fn pieces(&self) -> PyBitboard {
        PyBitboard::from_bitboard(self.position.pieces())
    }

    fn iter_pieces(&self) -> Vec<(PySquare, PyPiece)> {
        Square::iter()
            .filter_map(|sq| {
                let piece = self.position.piece_on(sq);
                (piece != Piece::NONE)
                    .then(|| (PySquare::from_square(sq), PyPiece::from_piece(piece)))
            })
            .collect()
    }

    fn pieces_by_type(&self, piece_type: &PyPieceType) -> PyBitboard {
        PyBitboard::from_bitboard(self.position.pieces_by_type(piece_type.inner()))
    }

    fn pieces_by_color(&self, color: &PyColor) -> PyBitboard {
        PyBitboard::from_bitboard(self.position.pieces_by_color(color.inner()))
    }

    fn pieces_for(&self, piece_type: &PyPieceType, color: &PyColor) -> PyBitboard {
        PyBitboard::from_bitboard(self.position.pieces_for(piece_type.inner(), color.inner()))
    }

    fn golds(&self) -> PyBitboard {
        PyBitboard::from_bitboard(self.position.golds())
    }

    fn bishop_horse(&self) -> PyBitboard {
        PyBitboard::from_bitboard(self.position.bishop_horse())
    }

    fn rook_dragon(&self) -> PyBitboard {
        PyBitboard::from_bitboard(self.position.rook_dragon())
    }

    fn attackers_to(&self, square: &PySquare, occupied: &PyBitboard) -> PyBitboard {
        PyBitboard::from_bitboard(self.position.attackers_to(square.inner(), occupied.inner()))
    }

    fn attackers_to_color(
        &self,
        color: &PyColor,
        square: &PySquare,
        occupied: &PyBitboard,
    ) -> PyBitboard {
        PyBitboard::from_bitboard(self.position.attackers_to_color(
            color.inner(),
            square.inner(),
            occupied.inner(),
        ))
    }

    fn attackers_to_color_current(&self, color: &PyColor, square: &PySquare) -> PyBitboard {
        PyBitboard::from_bitboard(
            self.position.attackers_to_color_current(color.inner(), square.inner()),
        )
    }

    fn is_attacked_by(&self, square: &PySquare, by_color: &PyColor) -> bool {
        self.position.is_attacked_by(square.inner(), by_color.inner())
    }

    fn attacks_by(&self, color: &PyColor, piece_type: &PyPieceType) -> PyBitboard {
        PyBitboard::from_bitboard(self.position.attacks_by(color.inner(), piece_type.inner()))
    }

    fn is_legal_move(&self, r#move: PyRef<'_, PyMove>) -> bool {
        let move_value = self.position.move32_from_move(r#move.inner);
        self.position.is_legal_move32(move_value)
    }

    fn is_legal_move32(&self, r#move: PyRef<'_, PyMove32>) -> bool {
        self.position.is_legal_move32(r#move.inner)
    }

    fn legal_moves(&self) -> Vec<PyMove> {
        let mut list = MoveList::new();
        generate_legal_all(&self.position, &mut list);
        collect_py_moves(&list)
    }

    fn legal_moves_move32(&self) -> Vec<PyMove32> {
        let mut list = MoveList::new();
        generate_legal_all(&self.position, &mut list);
        collect_py_moves32(&self.position, &list)
    }

    fn pseudo_legal_moves(&self) -> Vec<PyMove> {
        let mut list = MoveList::new();
        if self.position.is_in_check() {
            generate_moves::<Evasions>(&self.position, &mut list);
        } else {
            generate_moves::<NonEvasions>(&self.position, &mut list);
        }
        collect_py_moves(&list)
    }

    fn pseudo_legal_moves_move32(&self) -> Vec<PyMove32> {
        let mut list = MoveList::new();
        if self.position.is_in_check() {
            generate_moves::<Evasions>(&self.position, &mut list);
        } else {
            generate_moves::<NonEvasions>(&self.position, &mut list);
        }
        collect_py_moves32(&self.position, &list)
    }

    fn is_in_check(&self) -> bool {
        self.position.is_in_check()
    }

    fn is_mated(&self) -> bool {
        self.position.is_mated()
    }

    fn validate(&self) -> PyResult<()> {
        self.position
            .validate()
            .map_err(|err| PyValueError::new_err(validation_error_message(&err)))
    }

    fn is_valid(&self) -> bool {
        self.position.is_valid()
    }

    fn validate_all(&self) -> PyValidationReport {
        PyValidationReport::from_report(self.position.validate_all())
    }

    fn repetition_state(&self) -> PyRepetitionState {
        PyRepetitionState::from_state(self.position.repetition_state())
    }

    #[pyo3(signature = (threshold=3))]
    fn is_repetition(&self, threshold: u8) -> bool {
        self.position.is_repetition(threshold)
    }

    #[pyo3(signature = (last_move=None, scale=1.0))]
    fn to_svg(
        &self,
        py: Python<'_>,
        last_move: Option<&Bound<'_, PyAny>>,
        scale: f32,
    ) -> PyResult<Py<PySvg>> {
        let last_move = match last_move {
            Some(value) => Some(parse_move(value, &self.position)?),
            None => None,
        };
        let svg = self.position.to_svg(last_move, scale);
        Py::new(py, PySvg { inner: svg })
    }

    fn to_bod(&self) -> PyResult<String> {
        let last = self.position.last_move();
        if !last.is_normal() {
            return Ok(board_to_bod(&self.position));
        }
        let mut prev = self.position.clone();
        prev.undo_move32(last).map_err(|err| PyValueError::new_err(format!("{err:?}")))?;
        let move_text =
            move_to_bod(&prev, last).ok_or_else(|| PyValueError::new_err("invalid move"))?;
        Ok(format!(
            "{}\n手数＝{}　{}　まで",
            board_to_bod(&self.position),
            prev.game_ply(),
            move_text
        ))
    }

    fn to_csa(&self) -> String {
        board_to_csa(&self.position)
    }

    fn reset(&mut self) {
        self.position.set_hirate();
        self.position.init_stack();
    }
}

#[pymodule]
fn _rsshogi(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_type_classes(m)?;
    register_board_classes(m)?;
    register_move_classes(m)?;
    register_record_classes(m)?;
    register_book_classes(m)?;
    register_policy_constants(m)?;
    sazpack::register_sazpack(m)?;
    register_functions(m)?;
    add_dtype_module(_py, m)?;
    Ok(())
}

fn register_type_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyColor>()?;
    m.add_class::<PyPieceType>()?;
    m.add_class::<PyPiece>()?;
    m.add_class::<PySquare>()?;
    m.add_class::<PyBitboard>()?;
    m.add_class::<PyHand>()?;
    m.add_class::<PyMoveType>()?;
    m.add_class::<PyRepetitionState>()?;
    Ok(())
}

fn register_board_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySvg>()?;
    m.add_class::<PyValidationIssue>()?;
    m.add_class::<PyValidationReport>()?;
    m.add_class::<PyPositionState>()?;
    m.add_class::<PyBoard>()?;
    Ok(())
}

fn register_move_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyMove>()?;
    m.add_class::<PyAperyMove>()?;
    m.add_class::<PyMove32>()?;
    m.add_class::<PyAperyMove32>()?;
    Ok(())
}

fn register_record_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyGameResult>()?;
    m.add_class::<PyGameResultInfo>()?;
    m.add_class::<PyTimeControl>()?;
    m.add_class::<PyEngineInfo>()?;
    m.add_class::<PyMoveEntry>()?;
    m.add_class::<PySpecialMoveEntry>()?;
    m.add_class::<PyRecordEntry>()?;
    m.add_class::<PyUsiPositionParts>()?;
    m.add_class::<PyRecordNodeId>()?;
    m.add_class::<PyRecordMetadataKey>()?;
    m.add_class::<PyRecordMetadata>()?;
    m.add_class::<PyRecordEditor>()?;
    m.add_class::<PyRecord>()?;
    Ok(())
}

fn register_book_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    book::register_book_classes(m)
}

fn register_policy_constants(m: &Bound<'_, PyModule>) -> PyResult<()> {
    policy::register_policy_constants(m)
}

fn register_functions(m: &Bound<'_, PyModule>) -> PyResult<()> {
    book::register_book_functions(m)?;
    m.add_function(wrap_pyfunction!(parse_usi_position, m)?)?;
    m.add_function(wrap_pyfunction!(parse_usi_position_parts, m)?)?;
    m.add_function(wrap_pyfunction!(normalize_usi_position, m)?)?;
    m.add_function(wrap_pyfunction!(to_move, m)?)?;
    m.add_function(wrap_pyfunction!(pack::decode_pack, m)?)?;
    m.add_function(wrap_pyfunction!(pack::decode_pack_file, m)?)?;
    m.add_function(wrap_pyfunction!(pack::decode_sbinpack, m)?)?;
    m.add_function(wrap_pyfunction!(pack::decode_sbinpack_file, m)?)?;
    m.add_function(wrap_pyfunction!(pack::write_pack, m)?)?;
    m.add_function(wrap_pyfunction!(pack::write_pack_file, m)?)?;
    m.add_function(wrap_pyfunction!(pack::write_sbinpack, m)?)?;
    m.add_function(wrap_pyfunction!(pack::write_sbinpack_file, m)?)?;
    policy::register_policy_functions(m)?;
    Ok(())
}

pub(crate) fn sbinpack_chain_to_record(
    chain: &::rsshogi::records::formats::sbinpack::SbinpackChain,
) -> Result<Record, String> {
    let (result, ply) = unpack_ply_result(chain.stem.ply_result);
    let mut pos = Position::empty();
    pos.set_packed_sfen(&chain.stem.packed_sfen, false, ply).map_err(|err| format!("{err:?}"))?;
    let init_position_sfen = pos.to_sfen(None);

    let result = result.unwrap_or(GameResult::Invalid);
    let mut record = Record::new(init_position_sfen).map_err(|err| err.to_string())?;
    let mut parent = record.root_id();
    let mut replay = pos;
    for entry in &chain.moves {
        if !entry.mv.is_normal() || !replay.is_legal_move32(entry.mv) {
            return Err(format!("illegal sbinpack move: {}", entry.mv.to_move().to_usi()));
        }
        let mv = entry.mv.to_move();
        parent = record
            .append_move_with_annotation(
                parent,
                MoveEntry::new(mv),
                RecordAnnotation::new().with_engine_info(Some(
                    EngineInfo::new().with_eval(Some(Eval::from_i32(entry.eval))),
                )),
            )
            .map_err(|err| err.to_string())?;
        replay.apply_move32(entry.mv);
    }
    record
        .set_main_terminal(SpecialMoveEntry::new(special_move_from_game_result(result), result))
        .map_err(|err| err.to_string())?;
    Ok(record)
}

fn single_sbinpack_chain_from_bytes(
    bytes: &[u8],
) -> PyResult<::rsshogi::records::formats::sbinpack::SbinpackChain> {
    let mut file = deserialize_sbinpack_file(bytes)
        .map_err(|err| PyValueError::new_err(format!("sbinpack: {err:?}")))?;
    if file.chains.is_empty() {
        return Err(PyValueError::new_err("sbinpack: no chains found"));
    }
    if file.chains.len() > 1 {
        return Err(PyValueError::new_err("sbinpack: multiple chains not supported"));
    }
    Ok(file.chains.remove(0))
}

#[pyfunction]
fn parse_usi_position(position: &str) -> PyResult<PyBoard> {
    board::init();
    Ok(PyBoard { position: Box::new(parse_usi_position_inner(position)?) })
}

#[pyfunction]
fn parse_usi_position_parts(position: &str) -> PyResult<PyUsiPositionParts> {
    board::init();
    Ok(parse_usi_position_parts_inner(position)?.parts)
}

#[pyfunction]
fn normalize_usi_position(position: &str) -> PyResult<String> {
    let raw = position.trim();
    if raw.is_empty() {
        return Ok("startpos".to_string());
    }
    let has_moves_keyword = raw.split_whitespace().any(|token| token == "moves");
    let pos = parse_usi_position_inner(position)?;
    let sfen_token = pos.to_sfen(None);
    if sfen_token == InitialPosition::Standard.to_sfen() && !has_moves_keyword {
        Ok("startpos".to_string())
    } else {
        Ok(sfen_token)
    }
}

#[pyfunction]
fn to_move(r#move: &Bound<'_, PyAny>) -> PyResult<PyMove> {
    if let Ok(mv16) = r#move.extract::<PyRef<'_, PyMove>>() {
        return Ok(PyMove::from_move(mv16.inner));
    }
    if let Ok(mv32) = r#move.extract::<PyRef<'_, PyMove32>>() {
        return Ok(PyMove::from_move(mv32.inner.to_move()));
    }
    if let Some(value) = extract_u16_int_arg(r#move, "move")? {
        return Ok(PyMove::from_move(Move::from_raw(value)));
    }

    Err(PyTypeError::new_err("to_move() expects Move, Move32, or int"))
}

fn compare_eq(op: CompareOp, equal: Option<bool>, type_name: &str) -> PyResult<bool> {
    match op {
        CompareOp::Eq => Ok(equal.unwrap_or(false)),
        CompareOp::Ne => Ok(!equal.unwrap_or(false)),
        _ => Err(PyTypeError::new_err(format!("{type_name} only supports == and !="))),
    }
}

fn ensure_on_board_square(square: Square) -> PyResult<()> {
    if square.is_on_board() {
        Ok(())
    } else {
        Err(PyValueError::new_err("square must be on board"))
    }
}

fn raw_position_from_position_state(state: &RawPositionState) -> PyResult<Position> {
    board::init();
    Ok(board::position_from_position_state(state))
}

fn validation_error_message(err: &RawValidationError) -> String {
    match err {
        RawValidationError::NoKing(color) => format!("{color:?} king is missing"),
        RawValidationError::TwoKings(color) => format!("{color:?} has two or more kings"),
        RawValidationError::DoublePawn(file, color) => {
            format!("{color:?} has double pawn on file {file:?}")
        }
        RawValidationError::InvalidHandCount { piece, count } => {
            format!("invalid hand count for {piece:?}: {count}")
        }
        RawValidationError::InvalidPlacement(square, piece_type) => {
            format!("invalid placement of {piece_type:?} on {square:?}")
        }
    }
}

pub(crate) fn parse_move_arg(r#move: &Bound<'_, PyAny>) -> PyResult<Move> {
    if let Ok(mv16) = r#move.extract::<PyRef<'_, PyMove>>() {
        return Ok(mv16.inner);
    }
    if let Ok(mv32) = r#move.extract::<PyRef<'_, PyMove32>>() {
        return Ok(mv32.inner.to_move());
    }
    if let Some(value) = extract_u16_int_arg(r#move, "move")? {
        return Ok(Move::from_raw(value));
    }
    if let Ok(usi) = r#move.extract::<String>() {
        return Move::from_usi(&usi).ok_or_else(|| {
            PyValueError::new_err("move must be valid USI move when passed as str")
        });
    }
    Err(PyTypeError::new_err("move must be Move, Move32, int, or USI str"))
}

fn parse_apery_move_arg(r#move: &Bound<'_, PyAny>) -> PyResult<AperyMove> {
    if let Ok(mv16) = r#move.extract::<PyRef<'_, PyAperyMove>>() {
        return Ok(mv16.inner);
    }
    if let Ok(mv32) = r#move.extract::<PyRef<'_, PyAperyMove32>>() {
        return Ok(mv32.inner.to_apery_move());
    }
    if let Ok(mv16) = r#move.extract::<PyRef<'_, PyMove>>() {
        return Ok(mv16.inner.to_apery());
    }
    if let Ok(mv32) = r#move.extract::<PyRef<'_, PyMove32>>() {
        return Ok(mv32.inner.to_move().to_apery());
    }
    if let Some(value) = extract_u16_int_arg(r#move, "best_move")? {
        return Ok(AperyMove::from_raw(value));
    }
    if let Ok(usi) = r#move.extract::<String>() {
        return AperyMove::from_usi(&usi).ok_or_else(|| {
            PyValueError::new_err("best_move must be valid USI move when passed as str")
        });
    }
    Err(PyTypeError::new_err(
        "best_move must be AperyMove, AperyMove32, Move, Move32, int, or USI str",
    ))
}

fn parse_usi_position_inner(position: &str) -> PyResult<Position> {
    Ok(parse_usi_position_parts_inner(position)?.position)
}

struct ParsedUsiPosition {
    parts: PyUsiPositionParts,
    position: Position,
}

fn parse_usi_position_parts_inner(position: &str) -> PyResult<ParsedUsiPosition> {
    let text = position.trim();
    if text.is_empty() {
        return Err(PyValueError::new_err("USI position string must not be empty"));
    }

    let text = if let Some(rest) = text.strip_prefix("position ") { rest.trim() } else { text };

    let tokens: Vec<&str> = text.split_whitespace().collect();
    if tokens.is_empty() {
        return Err(PyValueError::new_err("USI position string must contain tokens"));
    }

    let mut pos = Position::empty();
    let initial_sfen: String;
    let move_start = match tokens[0] {
        "startpos" => {
            pos.set_hirate();
            initial_sfen = InitialPosition::Standard.to_sfen().to_string();
            1
        }
        "sfen" => {
            if tokens.len() < 5 {
                return Err(PyValueError::new_err(format!(
                    "invalid USI position (expected 'sfen <board> <turn> <hand> <ply>'): {position}"
                )));
            }
            let sfen = tokens[1..5].join(" ");
            pos.set_sfen(&sfen).map_err(|err| {
                PyValueError::new_err(format!("invalid USI position SFEN: {position} ({err})"))
            })?;
            initial_sfen = sfen;
            5
        }
        _ => {
            if tokens.len() < 4 {
                return Err(PyValueError::new_err(format!(
                    "invalid USI position token sequence: {position}"
                )));
            }
            let sfen = tokens[..4].join(" ");
            pos.set_sfen(&sfen).map_err(|err| {
                PyValueError::new_err(format!("invalid SFEN token: {position} ({err})"))
            })?;
            initial_sfen = sfen;
            4
        }
    };

    let mut moves = Vec::new();
    if move_start < tokens.len() {
        if tokens[move_start] != "moves" {
            return Err(PyValueError::new_err(format!(
                "unexpected token sequence in USI position: {position}"
            )));
        }
        for usi in &tokens[(move_start + 1)..] {
            let mv16 = Move::from_usi(usi).ok_or_else(|| {
                PyValueError::new_err(format!("invalid move '{usi}' in USI position: {position}"))
            })?;
            let mv = pos.move32_from_move(mv16);
            if !mv.is_normal() {
                return Err(PyValueError::new_err(format!(
                    "invalid move value '{usi}' in USI position: {position}"
                )));
            }
            if !pos.is_legal_move32(mv) {
                return Err(PyValueError::new_err(format!(
                    "illegal move '{usi}' in USI position: {position}"
                )));
            }
            pos.apply_move32(mv);
            moves.push(mv16);
        }
    }

    let parts = PyUsiPositionParts { initial_sfen, moves, final_sfen: pos.to_sfen(None) };
    Ok(ParsedUsiPosition { parts, position: pos })
}

fn parse_move_record_move(r#move: &Bound<'_, PyAny>) -> PyResult<Move> {
    let mv16 = parse_move_arg(r#move)?;
    if !mv16.is_normal() {
        return Err(PyValueError::new_err("move must be a normal move"));
    }
    Ok(mv16)
}

fn optional_index<T: Copy>(values: &Option<Vec<Option<T>>>, index: usize) -> Option<T> {
    values.as_ref().and_then(|values| values.get(index)).copied().flatten()
}

fn declaration_evaluation_to_pydict<'py>(
    py: Python<'py>,
    eval: ::rsshogi::board::position::DeclarationEvaluation,
) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new(py);
    out.set_item("rule", format!("{:?}", eval.rule))?;
    out.set_item("can_declare", eval.can_declare)?;
    match eval.detail {
        DeclarationDetail::NoRule => {
            out.set_item("detail_type", "NO_RULE")?;
        }
        DeclarationDetail::TryRule { king_square, try_square, can_reach, can_occupy, is_safe } => {
            out.set_item("detail_type", "TRY_RULE")?;
            out.set_item("king_square", king_square.map(PySquare::from_square))?;
            out.set_item("try_square", PySquare::from_square(try_square))?;
            out.set_item("can_reach", can_reach)?;
            out.set_item("can_occupy", can_occupy)?;
            out.set_item("is_safe", is_safe)?;
        }
        DeclarationDetail::PointRule {
            not_in_check,
            king_in_enemy_camp,
            pieces_in_camp,
            enough_pieces,
            points,
            required_points,
        } => {
            out.set_item("detail_type", "POINT_RULE")?;
            out.set_item("not_in_check", not_in_check)?;
            out.set_item("king_in_enemy_camp", king_in_enemy_camp)?;
            out.set_item("pieces_in_camp", pieces_in_camp)?;
            out.set_item("enough_pieces", enough_pieces)?;
            out.set_item("points", points)?;
            out.set_item("required_points", required_points)?;
        }
    }
    Ok(out)
}

fn move_delta_to_pydict<'py>(
    py: Python<'py>,
    mv: Move32,
    delta: MoveDelta32,
) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new(py);
    out.set_item("move", PyMove32 { inner: mv })?;
    out.set_item("move_usi", mv.to_usi())?;
    match delta {
        MoveDelta32::Drop { color, piece_type, to, hand_count_before } => {
            out.set_item("kind", "DROP")?;
            out.set_item("color", PyColor::from_color(color))?;
            out.set_item("piece_type", PyPieceType::from_piece_type(piece_type))?;
            out.set_item("to", PySquare::from_square(to))?;
            out.set_item("hand_count_before", hand_count_before)?;
        }
        MoveDelta32::Board { color, from, to, moved_piece_before, moved_piece_after, captured } => {
            out.set_item("kind", "BOARD")?;
            out.set_item("color", PyColor::from_color(color))?;
            out.set_item("from", PySquare::from_square(from))?;
            out.set_item("to", PySquare::from_square(to))?;
            out.set_item("moved_piece_before", PyPiece::from_piece(moved_piece_before))?;
            out.set_item("moved_piece_after", PyPiece::from_piece(moved_piece_after))?;
            if let Some(captured) = captured {
                let captured_dict = PyDict::new(py);
                captured_dict.set_item("piece", PyPiece::from_piece(captured.piece))?;
                captured_dict.set_item("hand_count_before", captured.hand_count_before)?;
                out.set_item("captured", captured_dict)?;
            } else {
                out.set_item("captured", py.None())?;
            }
        }
    }
    Ok(out)
}

fn normalize_record_moves(
    init_position_sfen: &str,
    moves: Vec<PyMoveEntry>,
) -> PyResult<Vec<PyMoveEntry>> {
    let mut pos = Position::empty();
    pos.set_sfen(init_position_sfen).map_err(|err| {
        PyValueError::new_err(format!("from_dict: invalid init_position_sfen: {err}"))
    })?;

    let mut normalized = Vec::with_capacity(moves.len());
    for (index, mv_record) in moves.into_iter().enumerate() {
        let mv16 = mv_record.inner.mv();
        if !mv16.is_normal() {
            return Err(PyValueError::new_err(format!(
                "from_dict: moves[{index}] is not a normal move"
            )));
        }

        let mv = pos.move32_from_move(mv16);
        if !mv.is_normal() {
            return Err(PyValueError::new_err(format!(
                "from_dict: moves[{index}] could not be normalized against current position"
            )));
        }
        if !pos.is_legal_move(mv16) {
            return Err(PyValueError::new_err(format!(
                "from_dict: moves[{index}] is illegal in current position"
            )));
        }

        normalized
            .push(PyMoveEntry { inner: MoveEntry::new(mv16), annotation: mv_record.annotation });
        pos.apply_move(mv16);
    }

    Ok(normalized)
}

pub(crate) fn parse_game_result_arg(value: &Bound<'_, PyAny>) -> PyResult<GameResult> {
    if let Ok(result) = value.extract::<PyRef<'_, PyGameResult>>() {
        return Ok(result.inner);
    }
    if let Ok(v) = value.extract::<u8>() {
        return game_result_from_value(v)
            .ok_or_else(|| PyValueError::new_err("invalid GameResult value"));
    }
    if let Ok(name) = value.extract::<String>() {
        return game_result_from_str(&name)
            .ok_or_else(|| PyValueError::new_err(format!("invalid GameResult name: {name}")));
    }
    Err(PyTypeError::new_err("result must be GameResult, int, or str"))
}

fn parse_hcpe_game_result_arg(value: &Bound<'_, PyAny>) -> PyResult<HcpeGameResult> {
    if let Ok(result) = value.extract::<PyRef<'_, PyGameResult>>() {
        return HcpeGameResult::try_from(result.inner)
            .map_err(|err| PyValueError::new_err(err.to_string()));
    }
    if let Ok(v) = value.extract::<i8>() {
        return HcpeGameResult::try_from(v).map_err(|err| PyValueError::new_err(err.to_string()));
    }
    if let Ok(v) = value.extract::<u8>() {
        return HcpeGameResult::try_from(v as i8)
            .map_err(|err| PyValueError::new_err(err.to_string()));
    }
    if let Ok(name) = value.extract::<String>() {
        let name = name.to_ascii_uppercase();
        return match name.as_str() {
            "DRAW" => Ok(HcpeGameResult::Draw),
            "BLACK_WIN" => Ok(HcpeGameResult::BlackWin),
            "WHITE_WIN" => Ok(HcpeGameResult::WhiteWin),
            _ => Err(PyValueError::new_err(format!("invalid hcpe game result name: {name}"))),
        };
    }
    Err(PyTypeError::new_err("game_result must be GameResult, int, or str"))
}

fn extract_u16_int_arg(value: &Bound<'_, PyAny>, arg_name: &str) -> PyResult<Option<u16>> {
    if value.is_instance_of::<PyInt>() {
        let text = value.str()?.to_str()?.to_owned();
        let parsed = text.parse::<i128>().map_err(|_| {
            PyValueError::new_err(format!("{arg_name} must be an integer fitting in uint16 range"))
        })?;
        return u16::try_from(parsed)
            .map(Some)
            .map_err(|_| PyValueError::new_err(format!("{arg_name} must fit in uint16 range")));
    }
    Ok(None)
}

fn special_move_from_name(kind: &str, unknown_name: Option<String>) -> PyResult<SpecialMove> {
    let kind = kind.to_ascii_uppercase();
    match kind.as_str() {
        "INTERRUPT" => Ok(SpecialMove::Interrupt),
        "RESIGN" => Ok(SpecialMove::Resign),
        "MAX_MOVES" => Ok(SpecialMove::MaxMoves),
        "IMPASSE" => Ok(SpecialMove::Impasse),
        "DRAW" => Ok(SpecialMove::Draw),
        "REPETITION_DRAW" => Ok(SpecialMove::RepetitionDraw),
        "MATE" => Ok(SpecialMove::Mate),
        "NO_MATE" => Ok(SpecialMove::NoMate),
        "TIMEOUT" => Ok(SpecialMove::Timeout),
        "WIN_BY_ILLEGAL_MOVE" => Ok(SpecialMove::WinByIllegalMove),
        "LOSE_BY_ILLEGAL_MOVE" => Ok(SpecialMove::LoseByIllegalMove),
        "WIN_BY_DECLARATION" => Ok(SpecialMove::WinByDeclaration),
        "WIN_BY_DEFAULT" => Ok(SpecialMove::WinByDefault),
        "LOSE_BY_DEFAULT" => Ok(SpecialMove::LoseByDefault),
        "TRY" => Ok(SpecialMove::Try),
        "UNKNOWN" => {
            Ok(SpecialMove::Unknown(unknown_name.unwrap_or_else(|| "UNKNOWN".to_string())))
        }
        _ => Err(PyValueError::new_err(format!("invalid SpecialMove kind: {kind}"))),
    }
}

fn dict_required_string(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<String> {
    let value = dict
        .get_item(key)?
        .ok_or_else(|| PyValueError::new_err(format!("from_dict: missing '{key}'")))?;
    value
        .extract::<String>()
        .map_err(|_| PyTypeError::new_err(format!("from_dict: '{key}' must be str")))
}

fn move_records_from_sequence(
    seq: &Bound<'_, PyAny>,
    mode: ParseMode,
) -> PyResult<Vec<PyMoveEntry>> {
    let mut moves = Vec::new();
    for item in seq.try_iter()? {
        moves.push(move_record_from_pyany(&item?, mode)?);
    }
    Ok(moves)
}

fn move_record_from_pyany(value: &Bound<'_, PyAny>, mode: ParseMode) -> PyResult<PyMoveEntry> {
    if let Ok(mv) = value.extract::<PyRef<'_, PyMoveEntry>>() {
        return Ok(PyMoveEntry { inner: mv.inner.clone(), annotation: mv.annotation.clone() });
    }
    if !value.is_instance_of::<PyDict>() {
        let mv = parse_move_record_move(value)?;
        return Ok(PyMoveEntry { inner: MoveEntry::new(mv), annotation: RecordAnnotation::new() });
    }
    let dict = value
        .cast::<PyDict>()
        .map_err(|_| PyTypeError::new_err("move must be MoveEntry or dict"))?;
    if mode.is_strict() {
        for (key_any, _) in dict.iter() {
            let key = key_any
                .extract::<String>()
                .map_err(|_| PyTypeError::new_err("move dict keys must be str"))?;
            if key != "move" && key != "comment" && key != "time_ms" && key != "engine_info" {
                return Err(PyValueError::new_err(format!(
                    "move dict: unknown key '{key}' in strict mode"
                )));
            }
        }
    }
    let mv_any =
        dict.get_item("move")?.ok_or_else(|| PyValueError::new_err("move dict: missing 'move'"))?;
    let mv = parse_move_record_move(&mv_any)?;
    let comment = dict
        .get_item("comment")?
        .map_or(Ok(None), |v| v.extract::<Option<String>>())
        .map_err(|_| PyTypeError::new_err("move dict: 'comment' must be str or None"))?;
    let time_ms = dict
        .get_item("time_ms")?
        .map_or(Ok(None), |v| v.extract::<Option<u32>>())
        .map_err(|_| PyTypeError::new_err("move dict: 'time_ms' must be int or None"))?;
    let engine_info = match dict.get_item("engine_info")? {
        Some(v) => {
            if v.is_none() {
                None
            } else {
                Some(move_engine_info_from_pyany(&v, mode)?)
            }
        }
        _ => None,
    };

    Ok(PyMoveEntry {
        inner: MoveEntry::new(mv),
        annotation: RecordAnnotation::new()
            .with_comment(comment)
            .with_elapsed_ms(time_ms)
            .with_engine_info(engine_info),
    })
}

fn move_record_to_pydict<'py>(
    py: Python<'py>,
    mv: &MoveEntry,
    annotation: &RecordAnnotation,
) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new(py);
    out.set_item("move", mv.mv().to_usi())?;
    out.set_item("comment", annotation.comment())?;
    out.set_item("time_ms", annotation.elapsed_ms())?;
    out.set_item(
        "engine_info",
        annotation.engine_info().map(|value| move_engine_info_to_pydict(py, value)).transpose()?,
    )?;
    Ok(out)
}

fn move_engine_info_from_pyany(value: &Bound<'_, PyAny>, mode: ParseMode) -> PyResult<EngineInfo> {
    if let Ok(engine_info) = value.extract::<PyRef<'_, PyEngineInfo>>() {
        return Ok(engine_info.inner.clone());
    }
    let dict = value
        .cast::<PyDict>()
        .map_err(|_| PyTypeError::new_err("engine_info must be EngineInfo or dict"))?;
    if mode.is_strict() {
        for (key_any, _) in dict.iter() {
            let key = key_any
                .extract::<String>()
                .map_err(|_| PyTypeError::new_err("engine_info dict keys must be str"))?;
            if key != "eval"
                && key != "depth"
                && key != "nodes"
                && key != "seldepth"
                && key != "extras"
            {
                return Err(PyValueError::new_err(format!(
                    "engine_info dict: unknown key '{key}' in strict mode"
                )));
            }
        }
    }
    let eval = dict
        .get_item("eval")?
        .map_or(Ok(None), |v| v.extract::<Option<i32>>())
        .map_err(|_| PyTypeError::new_err("engine_info dict: 'eval' must be int or None"))?;
    let depth = dict
        .get_item("depth")?
        .map_or(Ok(None), |v| v.extract::<Option<u16>>())
        .map_err(|_| PyTypeError::new_err("engine_info dict: 'depth' must be int or None"))?;
    let nodes = dict
        .get_item("nodes")?
        .map_or(Ok(None), |v| v.extract::<Option<u64>>())
        .map_err(|_| PyTypeError::new_err("engine_info dict: 'nodes' must be int or None"))?;
    let seldepth = dict
        .get_item("seldepth")?
        .map_or(Ok(None), |v| v.extract::<Option<u16>>())
        .map_err(|_| PyTypeError::new_err("engine_info dict: 'seldepth' must be int or None"))?;
    let extras = match dict.get_item("extras")? {
        Some(v) => {
            if v.is_none() {
                None
            } else {
                Some(parse_engine_extras_from_pyany(&v)?)
            }
        }
        _ => None,
    };

    let mut info = EngineInfo::new()
        .with_eval(eval.map(Eval::from_i32))
        .with_depth(depth)
        .with_nodes(nodes)
        .with_seldepth(seldepth);
    if let Some(extras) = extras {
        for (key, value) in extras {
            info.set_extra(key, value);
        }
    }
    Ok(info)
}

fn move_engine_info_to_pydict<'py>(
    py: Python<'py>,
    info: &EngineInfo,
) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new(py);
    out.set_item("eval", info.eval().map(Eval::to_i32))?;
    out.set_item("depth", info.depth())?;
    out.set_item("nodes", info.nodes())?;
    out.set_item("seldepth", info.seldepth())?;
    out.set_item("extras", move_engine_extras_to_pydict(py, info.extras())?)?;
    Ok(out)
}

fn parse_engine_extras_from_pyany(
    value: &Bound<'_, PyAny>,
) -> PyResult<HashMap<String, EngineExtraValue>> {
    let dict = value
        .cast::<PyDict>()
        .map_err(|_| PyTypeError::new_err("extras must be dict[str, str|int|float|bool]"))?;
    let mut extras = HashMap::new();
    for (key, value) in dict.iter() {
        let key =
            key.extract::<String>().map_err(|_| PyTypeError::new_err("extras keys must be str"))?;
        extras.insert(key, move_engine_extra_from_pyany(&value)?);
    }
    Ok(extras)
}

fn move_engine_extra_from_pyany(value: &Bound<'_, PyAny>) -> PyResult<EngineExtraValue> {
    if value.is_instance_of::<PyBool>() {
        return Ok(EngineExtraValue::Bool(value.extract::<bool>()?));
    }
    if let Ok(int_value) = value.extract::<i64>() {
        return Ok(EngineExtraValue::Int(int_value));
    }
    if let Ok(float_value) = value.extract::<f64>() {
        if !float_value.is_finite() {
            return Err(PyValueError::new_err("extras float values must be finite"));
        }
        return Ok(EngineExtraValue::Float(float_value.into()));
    }
    if let Ok(string_value) = value.extract::<String>() {
        return Ok(EngineExtraValue::String(string_value));
    }
    Err(PyTypeError::new_err("extras values must be str, int, float, or bool"))
}

fn move_engine_extra_to_pyobject(py: Python<'_>, value: &EngineExtraValue) -> PyResult<Py<PyAny>> {
    let out = PyDict::new(py);
    match value {
        EngineExtraValue::String(v) => out.set_item("value", v)?,
        EngineExtraValue::Int(v) => out.set_item("value", *v)?,
        EngineExtraValue::Float(v) => out.set_item("value", v.into_inner())?,
        EngineExtraValue::Bool(v) => out.set_item("value", *v)?,
    }
    let value = out
        .get_item("value")?
        .ok_or_else(|| PyValueError::new_err("failed to convert extras value"))?;
    Ok(value.unbind())
}

fn move_engine_extras_to_pydict<'py>(
    py: Python<'py>,
    extras: &HashMap<String, EngineExtraValue>,
) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new(py);
    for (key, value) in extras {
        out.set_item(key, move_engine_extra_to_pyobject(py, value)?)?;
    }
    Ok(out)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ParseMode {
    Strict,
    Permissive,
}

impl ParseMode {
    const fn from_strict(strict: bool) -> Self {
        if strict { Self::Strict } else { Self::Permissive }
    }

    const fn is_strict(self) -> bool {
        matches!(self, Self::Strict)
    }
}

fn is_removed_metadata_key(key: &str) -> bool {
    matches!(
        key,
        "black_player_name"
            | "white_player_name"
            | "time_control_black"
            | "time_control_white"
            | "time_specs"
    )
}

fn metadata_from_pyany(value: &Bound<'_, PyAny>, mode: ParseMode) -> PyResult<RecordMetadata> {
    if let Ok(metadata) = value.extract::<PyRef<'_, PyRecordMetadata>>() {
        return Ok(metadata.inner.clone());
    }
    let dict = value
        .cast::<PyDict>()
        .map_err(|_| PyTypeError::new_err("metadata must be RecordMetadata or dict"))?;
    let mut builder = RecordMetadata::builder();
    let mut attributes: HashMap<String, String> = HashMap::new();

    if mode.is_strict() {
        for (key_any, _) in dict.iter() {
            let key = key_any
                .extract::<String>()
                .map_err(|_| PyTypeError::new_err("metadata dict keys must be str"))?;
            if key != "event"
                && key != "site"
                && key != "black_player"
                && key != "white_player"
                && key != "game_name"
                && key != "game_type"
                && key != "time_control"
                && key != "black_time_control"
                && key != "white_time_control"
                && key != "max_moves"
                && key != "impasse_rule"
                && key != "start_date"
                && key != "end_date"
                && key != "updated_date"
                && key != "comment"
                && key != "attributes"
            {
                return Err(PyValueError::new_err(format!(
                    "metadata dict: unknown key '{key}' in strict mode"
                )));
            }
        }
    }

    if let Some(v) = dict.get_item("event")? {
        builder.event(
            v.extract::<Option<String>>()
                .map_err(|_| PyTypeError::new_err("metadata dict: 'event' must be str or None"))?,
        );
    }
    if let Some(v) = dict.get_item("site")? {
        builder.site(
            v.extract::<Option<String>>()
                .map_err(|_| PyTypeError::new_err("metadata dict: 'site' must be str or None"))?,
        );
    }
    if let Some(v) = dict.get_item("black_player")? {
        builder.black_player(v.extract::<Option<String>>().map_err(|_| {
            PyTypeError::new_err("metadata dict: 'black_player' must be str or None")
        })?);
    }
    if let Some(v) = dict.get_item("white_player")? {
        builder.white_player(v.extract::<Option<String>>().map_err(|_| {
            PyTypeError::new_err("metadata dict: 'white_player' must be str or None")
        })?);
    }
    if let Some(v) = dict.get_item("game_name")? {
        builder.game_name(
            v.extract::<Option<String>>().map_err(|_| {
                PyTypeError::new_err("metadata dict: 'game_name' must be str or None")
            })?,
        );
    }
    if let Some(v) = dict.get_item("game_type")? {
        builder.game_type(
            v.extract::<Option<String>>().map_err(|_| {
                PyTypeError::new_err("metadata dict: 'game_type' must be str or None")
            })?,
        );
    }
    if let Some(v) = dict.get_item("max_moves")? {
        builder.max_moves(
            v.extract::<Option<u32>>().map_err(|_| {
                PyTypeError::new_err("metadata dict: 'max_moves' must be int or None")
            })?,
        );
    }
    if let Some(v) = dict.get_item("impasse_rule")? {
        builder.impasse_rule(v.extract::<Option<String>>().map_err(|_| {
            PyTypeError::new_err("metadata dict: 'impasse_rule' must be str or None")
        })?);
    }
    if let Some(v) = dict.get_item("start_date")? {
        builder.start_date(v.extract::<Option<String>>().map_err(|_| {
            PyTypeError::new_err("metadata dict: 'start_date' must be str or None")
        })?);
    }
    if let Some(v) = dict.get_item("end_date")? {
        builder.end_date(
            v.extract::<Option<String>>().map_err(|_| {
                PyTypeError::new_err("metadata dict: 'end_date' must be str or None")
            })?,
        );
    }
    if let Some(v) = dict.get_item("updated_date")? {
        builder.updated_date(v.extract::<Option<String>>().map_err(|_| {
            PyTypeError::new_err("metadata dict: 'updated_date' must be str or None")
        })?);
    }
    if let Some(v) = dict.get_item("comment")? {
        builder.comment(
            v.extract::<Option<String>>().map_err(|_| {
                PyTypeError::new_err("metadata dict: 'comment' must be str or None")
            })?,
        );
    }
    if let Some(v) = dict.get_item("time_control")? {
        let tc = parse_time_control_option(v)?;
        builder.time_control(tc);
    }
    if let Some(v) = dict.get_item("black_time_control")? {
        let tc = parse_time_control_option(v)?;
        builder.black_time_control(tc);
    }
    if let Some(v) = dict.get_item("white_time_control")? {
        let tc = parse_time_control_option(v)?;
        builder.white_time_control(tc);
    }
    if let Some(v) = dict.get_item("attributes")? {
        attributes = v.extract::<HashMap<String, String>>().map_err(|_| {
            PyTypeError::new_err("metadata dict: 'attributes' must be dict[str, str]")
        })?;
    }
    if !mode.is_strict() {
        for (key_any, value) in dict.iter() {
            let key = key_any
                .extract::<String>()
                .map_err(|_| PyTypeError::new_err("metadata dict keys must be str"))?;
            if key == "event"
                || key == "site"
                || key == "black_player"
                || key == "white_player"
                || key == "game_name"
                || key == "game_type"
                || key == "time_control"
                || key == "black_time_control"
                || key == "white_time_control"
                || key == "max_moves"
                || key == "impasse_rule"
                || key == "start_date"
                || key == "end_date"
                || key == "updated_date"
                || key == "comment"
                || key == "attributes"
            {
                continue;
            }
            if is_removed_metadata_key(&key) {
                return Err(PyValueError::new_err(format!(
                    "metadata dict: key '{key}' was removed; use black_time_control/white_time_control/time_control"
                )));
            }
            if value.is_none() {
                attributes.remove(&key);
                continue;
            }
            let raw = value.extract::<String>().map_err(|_| {
                PyTypeError::new_err(format!(
                    "metadata dict: unknown key '{key}' must be str or None in permissive mode"
                ))
            })?;
            attributes.insert(key, raw);
        }
    }

    for (key, value) in attributes {
        builder.add_attribute(key, value);
    }
    Ok(builder.build())
}

fn parse_time_control_option(value: Bound<'_, PyAny>) -> PyResult<Option<TimeControl>> {
    if value.is_none() {
        return Ok(None);
    }
    if let Ok(tc) = value.extract::<PyRef<'_, PyTimeControl>>() {
        return Ok(Some(tc.inner));
    }
    if let Ok(spec) = value.extract::<String>() {
        return TimeControl::from_spec(&spec)
            .map(Some)
            .ok_or_else(|| PyValueError::new_err("invalid time control spec"));
    }
    let dict = value.cast::<PyDict>().map_err(|_| {
        PyTypeError::new_err("time control must be TimeControl, str, dict, or None")
    })?;
    let base = dict
        .get_item("base_seconds")?
        .map_or(Ok(0), |v| v.extract::<u32>())
        .map_err(|_| PyTypeError::new_err("'base_seconds' must be int"))?;
    let byoyomi = dict
        .get_item("byoyomi_seconds")?
        .map_or(Ok(0), |v| v.extract::<u32>())
        .map_err(|_| PyTypeError::new_err("'byoyomi_seconds' must be int"))?;
    let increment = dict
        .get_item("increment_seconds")?
        .map_or(Ok(0), |v| v.extract::<u32>())
        .map_err(|_| PyTypeError::new_err("'increment_seconds' must be int"))?;
    Ok(Some(TimeControl::new(base, byoyomi, increment)))
}

fn apply_metadata_patch(
    base: &RecordMetadata,
    patch: &Bound<'_, PyDict>,
    mode: ParseMode,
) -> PyResult<RecordMetadata> {
    let mut event = base.event().map(ToOwned::to_owned);
    let mut site = base.site().map(ToOwned::to_owned);
    let mut black_player = base.black_player().map(ToOwned::to_owned);
    let mut white_player = base.white_player().map(ToOwned::to_owned);
    let mut game_name = base.game_name().map(ToOwned::to_owned);
    let mut game_type = base.game_type().map(ToOwned::to_owned);
    let mut time_control = base.time_control().copied();
    let mut black_time_control = base.black_time_control().copied();
    let mut white_time_control = base.white_time_control().copied();
    let mut max_moves = base.max_moves();
    let mut impasse_rule = base.impasse_rule().map(ToOwned::to_owned);
    let mut start_date = base.start_date().map(ToOwned::to_owned);
    let mut end_date = base.end_date().map(ToOwned::to_owned);
    let mut updated_date = base.updated_date().map(ToOwned::to_owned);
    let mut comment = base.comment().map(ToOwned::to_owned);
    let mut attributes = base.attributes().clone();

    for (key_any, value) in patch.iter() {
        let key = key_any
            .extract::<String>()
            .map_err(|_| PyTypeError::new_err("update_metadata: patch keys must be str"))?;
        match key.as_str() {
            "event" => {
                event = value.extract::<Option<String>>().map_err(|_| {
                    PyTypeError::new_err("update_metadata: 'event' must be str or None")
                })?;
            }
            "site" => {
                site = value.extract::<Option<String>>().map_err(|_| {
                    PyTypeError::new_err("update_metadata: 'site' must be str or None")
                })?;
            }
            "black_player" => {
                black_player = value.extract::<Option<String>>().map_err(|_| {
                    PyTypeError::new_err("update_metadata: 'black_player' must be str or None")
                })?;
            }
            "white_player" => {
                white_player = value.extract::<Option<String>>().map_err(|_| {
                    PyTypeError::new_err("update_metadata: 'white_player' must be str or None")
                })?;
            }
            "game_name" => {
                game_name = value.extract::<Option<String>>().map_err(|_| {
                    PyTypeError::new_err("update_metadata: 'game_name' must be str or None")
                })?;
            }
            "game_type" => {
                game_type = value.extract::<Option<String>>().map_err(|_| {
                    PyTypeError::new_err("update_metadata: 'game_type' must be str or None")
                })?;
            }
            "time_control" => {
                time_control = parse_time_control_option(value)?;
            }
            "black_time_control" => {
                black_time_control = parse_time_control_option(value)?;
            }
            "white_time_control" => {
                white_time_control = parse_time_control_option(value)?;
            }
            "max_moves" => {
                max_moves = value.extract::<Option<u32>>().map_err(|_| {
                    PyTypeError::new_err("update_metadata: 'max_moves' must be int or None")
                })?;
            }
            "impasse_rule" => {
                impasse_rule = value.extract::<Option<String>>().map_err(|_| {
                    PyTypeError::new_err("update_metadata: 'impasse_rule' must be str or None")
                })?;
            }
            "start_date" => {
                start_date = value.extract::<Option<String>>().map_err(|_| {
                    PyTypeError::new_err("update_metadata: 'start_date' must be str or None")
                })?;
            }
            "end_date" => {
                end_date = value.extract::<Option<String>>().map_err(|_| {
                    PyTypeError::new_err("update_metadata: 'end_date' must be str or None")
                })?;
            }
            "updated_date" => {
                updated_date = value.extract::<Option<String>>().map_err(|_| {
                    PyTypeError::new_err("update_metadata: 'updated_date' must be str or None")
                })?;
            }
            "comment" => {
                comment = value.extract::<Option<String>>().map_err(|_| {
                    PyTypeError::new_err("update_metadata: 'comment' must be str or None")
                })?;
            }
            "attributes" => {
                if value.is_none() {
                    attributes.clear();
                } else {
                    attributes = value
                        .extract::<HashMap<String, String>>()
                        .map_err(|_| {
                            PyTypeError::new_err(
                                "update_metadata: 'attributes' must be dict[str, str] or None",
                            )
                        })?
                        .into_iter()
                        .collect::<BTreeMap<_, _>>();
                }
            }
            _ => {
                if is_removed_metadata_key(&key) {
                    return Err(PyValueError::new_err(format!(
                        "update_metadata: key '{key}' was removed; use black_time_control/white_time_control/time_control"
                    )));
                }
                if mode.is_strict() {
                    return Err(PyValueError::new_err(format!(
                        "update_metadata: unknown key '{key}'"
                    )));
                }
                if value.is_none() {
                    attributes.remove(&key);
                } else {
                    let mapped = value.extract::<String>().map_err(|_| {
                        PyTypeError::new_err(format!(
                            "update_metadata: unknown key '{key}' must be str or None in permissive mode"
                        ))
                    })?;
                    attributes.insert(key, mapped);
                }
            }
        }
    }

    let mut builder = RecordMetadata::builder();
    builder
        .event(event)
        .site(site)
        .black_player(black_player)
        .white_player(white_player)
        .game_name(game_name)
        .game_type(game_type)
        .time_control(time_control)
        .black_time_control(black_time_control)
        .white_time_control(white_time_control)
        .max_moves(max_moves)
        .impasse_rule(impasse_rule)
        .start_date(start_date)
        .end_date(end_date)
        .updated_date(updated_date)
        .comment(comment);
    for (key, value) in attributes {
        builder.add_attribute(key, value);
    }
    Ok(builder.build())
}

fn metadata_to_pydict<'py>(
    py: Python<'py>,
    metadata: &RecordMetadata,
) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new(py);
    out.set_item("event", metadata.event())?;
    out.set_item("site", metadata.site())?;
    out.set_item("black_player", metadata.black_player())?;
    out.set_item("white_player", metadata.white_player())?;
    out.set_item("game_name", metadata.game_name())?;
    out.set_item("game_type", metadata.game_type())?;
    out.set_item("max_moves", metadata.max_moves())?;
    out.set_item("impasse_rule", metadata.impasse_rule())?;
    out.set_item("start_date", metadata.start_date())?;
    out.set_item("end_date", metadata.end_date())?;
    out.set_item("updated_date", metadata.updated_date())?;
    out.set_item("comment", metadata.comment())?;

    if let Some(tc) = metadata.time_control() {
        out.set_item("time_control", time_control_to_pydict(py, *tc)?)?;
    } else {
        out.set_item("time_control", py.None())?;
    }
    if let Some(tc) = metadata.black_time_control() {
        out.set_item("black_time_control", time_control_to_pydict(py, *tc)?)?;
    } else {
        out.set_item("black_time_control", py.None())?;
    }
    if let Some(tc) = metadata.white_time_control() {
        out.set_item("white_time_control", time_control_to_pydict(py, *tc)?)?;
    } else {
        out.set_item("white_time_control", py.None())?;
    }

    out.set_item("attributes", metadata.attributes().clone())?;
    Ok(out)
}

type ResultPayload = (GameResult, Option<String>, Option<u32>, Option<String>);

fn parse_result_payload(value: &Bound<'_, PyAny>, mode: ParseMode) -> PyResult<ResultPayload> {
    match value.cast::<PyDict>() {
        Ok(result_dict) => {
            if mode.is_strict() {
                for (key_any, _) in result_dict.iter() {
                    let key = key_any
                        .extract::<String>()
                        .map_err(|_| PyTypeError::new_err("from_dict: result keys must be str"))?;
                    if key != "result"
                        && key != "ply_count"
                        && key != "reason"
                        && key != "end_time_ms"
                        && key != "end_comment"
                    {
                        return Err(PyValueError::new_err(format!(
                            "from_dict: unknown result key '{key}' in strict mode"
                        )));
                    }
                }
            }
            let result_value_any = result_dict
                .get_item("result")?
                .ok_or_else(|| PyValueError::new_err("from_dict: result.result is required"))?;
            let result_value = parse_game_result_arg(&result_value_any)?;
            let reason = result_dict
                .get_item("reason")?
                .map_or(Ok(None), |v| v.extract::<Option<String>>())
                .map_err(|_| {
                    PyTypeError::new_err("from_dict: result.reason must be str or None")
                })?;
            let end_time_ms = result_dict
                .get_item("end_time_ms")?
                .map_or(Ok(None), |v| v.extract::<Option<u32>>())
                .map_err(|_| {
                    PyTypeError::new_err("from_dict: result.end_time_ms must be int or None")
                })?;
            let end_comment = result_dict
                .get_item("end_comment")?
                .map_or(Ok(None), |v| v.extract::<Option<String>>())
                .map_err(|_| {
                    PyTypeError::new_err("from_dict: result.end_comment must be str or None")
                })?;
            Ok((result_value, reason, end_time_ms, end_comment))
        }
        _ => {
            let result_value = parse_game_result_arg(value)?;
            Ok((result_value, None, None, None))
        }
    }
}

fn terminal_from_result_payload(
    result_value: GameResult,
    reason: Option<String>,
    end_time_ms: Option<u32>,
    end_comment: Option<String>,
) -> Option<PySpecialMoveEntry> {
    if result_value == GameResult::Invalid
        && reason.is_none()
        && end_time_ms.is_none()
        && end_comment.is_none()
    {
        return None;
    }

    Some(PySpecialMoveEntry {
        inner: SpecialMoveEntry::new(special_move_from_game_result(result_value), result_value)
            .with_raw(reason),
        annotation: RecordAnnotation::new().with_elapsed_ms(end_time_ms).with_comment(end_comment),
    })
}

fn game_result_info_to_pydict<'py>(
    py: Python<'py>,
    info: &PyGameResultInfo,
) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new(py);
    out.set_item("result", info.result as u8)?;
    out.set_item("ply_count", info.ply_count)?;
    out.set_item("reason", info.reason.clone())?;
    out.set_item("end_time_ms", info.end_time_ms)?;
    out.set_item("end_comment", info.end_comment.clone())?;
    Ok(out)
}

fn game_result_info_from_record(record: &Record) -> PyGameResultInfo {
    let terminal_node = record.main_terminal_node().map(|node_id| record.node(node_id));
    PyGameResultInfo {
        result: record.result(),
        ply_count: record.move_count(),
        reason: terminal_node
            .and_then(|node| node.special().and_then(SpecialMoveEntry::raw))
            .map(ToOwned::to_owned),
        end_time_ms: terminal_node.and_then(|node| node.time_ms()),
        end_comment: terminal_node.and_then(|node| node.comment()).map(ToOwned::to_owned),
    }
}

fn time_control_to_pydict<'py>(py: Python<'py>, tc: TimeControl) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new(py);
    out.set_item("base_seconds", tc.base_seconds())?;
    out.set_item("byoyomi_seconds", tc.byoyomi_seconds())?;
    out.set_item("increment_seconds", tc.increment_seconds())?;
    Ok(out)
}

fn parse_move(r#move: &Bound<'_, PyAny>, position: &Position) -> PyResult<Move32> {
    let move_value = match r#move.extract::<PyRef<'_, PyMove>>() {
        Ok(mv16) => position.move32_from_move(mv16.inner),
        _ => match r#move.extract::<PyRef<'_, PyMove32>>() {
            Ok(mv32) => mv32.inner,
            _ => {
                return Err(PyTypeError::new_err("move must be Move32 or Move"));
            }
        },
    };

    if !move_value.is_normal() {
        return Err(PyValueError::new_err("invalid move value"));
    }

    Ok(move_value)
}

fn psfen_from_vec(data: Vec<u8>) -> PyResult<[u8; 32]> {
    if data.len() != 32 {
        return Err(PyValueError::new_err("psfen must be 32 bytes"));
    }
    let mut packed = [0u8; 32];
    packed.copy_from_slice(&data);
    Ok(packed)
}

fn extract_psfen_bytes(psfen: &Bound<'_, PyAny>) -> PyResult<[u8; 32]> {
    if let Ok(data) = psfen.extract::<Vec<u8>>() {
        return psfen_from_vec(data);
    }

    if let Ok(field) = psfen.get_item("sfen")
        && let Ok(data) = field.extract::<Vec<u8>>()
    {
        return psfen_from_vec(data);
    }

    if let Ok(first) = psfen.get_item(0) {
        if let Ok(field) = first.get_item("sfen")
            && let Ok(data) = field.extract::<Vec<u8>>()
        {
            return psfen_from_vec(data);
        }
        if let Ok(data) = first.extract::<Vec<u8>>() {
            return psfen_from_vec(data);
        }
    }

    Err(PyTypeError::new_err(
        "psfen must be bytes-like, uint8[32], or PackedSfen ndarray with 'sfen' field",
    ))
}

fn hcp_from_vec(data: Vec<u8>) -> PyResult<[u8; 32]> {
    if data.len() != 32 {
        return Err(PyValueError::new_err("hcp must be 32 bytes"));
    }
    let mut packed = [0u8; 32];
    packed.copy_from_slice(&data);
    Ok(packed)
}

fn extract_hcp_bytes(hcp: &Bound<'_, PyAny>) -> PyResult<[u8; 32]> {
    if let Ok(data) = hcp.extract::<Vec<u8>>() {
        return hcp_from_vec(data);
    }

    if let Ok(field) = hcp.get_item("hcp")
        && let Ok(data) = field.extract::<Vec<u8>>()
    {
        return hcp_from_vec(data);
    }

    if let Ok(first) = hcp.get_item(0) {
        if let Ok(field) = first.get_item("hcp")
            && let Ok(data) = field.extract::<Vec<u8>>()
        {
            return hcp_from_vec(data);
        }
        if let Ok(data) = first.extract::<Vec<u8>>() {
            return hcp_from_vec(data);
        }
    }

    Err(PyTypeError::new_err(
        "hcp must be bytes-like, uint8[32], or HuffmanCodedPos ndarray with 'hcp' field",
    ))
}

fn fill_psfen_output(out: &Bound<'_, PyAny>, data: &[u8; 32]) -> PyResult<()> {
    if let Ok(len) = out.len()
        && len == 32
    {
        for (idx, byte) in data.iter().copied().enumerate() {
            out.set_item(idx, byte)?;
        }
        return Ok(());
    }

    if let Ok(first) = out.get_item(0)
        && let Ok(field) = first.get_item("sfen")
        && field.len().ok() == Some(32)
    {
        for (idx, byte) in data.iter().copied().enumerate() {
            field.set_item(idx, byte)?;
        }
        return Ok(());
    }

    Err(PyTypeError::new_err(
        "out must be writable uint8[32] or PackedSfen ndarray with 'sfen' field",
    ))
}

fn fill_hcp_output(out: &Bound<'_, PyAny>, data: &[u8; 32]) -> PyResult<()> {
    if let Ok(len) = out.len()
        && len == 32
    {
        for (idx, byte) in data.iter().copied().enumerate() {
            out.set_item(idx, byte)?;
        }
        return Ok(());
    }

    if let Ok(first) = out.get_item(0)
        && let Ok(field) = first.get_item("hcp")
        && field.len().ok() == Some(32)
    {
        for (idx, byte) in data.iter().copied().enumerate() {
            field.set_item(idx, byte)?;
        }
        return Ok(());
    }

    Err(PyTypeError::new_err(
        "out must be writable uint8[32] or HuffmanCodedPos ndarray with 'hcp' field",
    ))
}

fn fill_psfen_value_output(
    out: &Bound<'_, PyAny>,
    data: &[u8; 40],
    score: i16,
    mv_raw: u16,
    game_ply: u16,
    game_result: i8,
) -> PyResult<()> {
    if let Ok(len) = out.len()
        && len == 40
    {
        for (idx, byte) in data.iter().copied().enumerate() {
            out.set_item(idx, byte)?;
        }
        return Ok(());
    }

    if let Ok(first) = out.get_item(0)
        && fill_psfen_value_fields(&first, data, score, mv_raw, game_ply, game_result).is_ok()
    {
        return Ok(());
    }

    if fill_psfen_value_fields(out, data, score, mv_raw, game_ply, game_result).is_ok() {
        return Ok(());
    }

    Err(PyTypeError::new_err(
        "out must be writable uint8[40] or PackedSfenValue ndarray with fields",
    ))
}

fn fill_psfen_value_fields(
    target: &Bound<'_, PyAny>,
    data: &[u8; 40],
    score: i16,
    mv_raw: u16,
    game_ply: u16,
    game_result: i8,
) -> PyResult<()> {
    let field = target.get_item("sfen")?;
    if field.len().ok() != Some(32) {
        return Err(PyTypeError::new_err("sfen field must have length 32"));
    }
    for (idx, byte) in data[..32].iter().copied().enumerate() {
        field.set_item(idx, byte)?;
    }
    target.set_item("score", score)?;
    target.set_item("move", mv_raw)?;
    target.set_item("game_ply", game_ply)?;
    target.set_item("game_result", game_result)?;
    let _ = target.set_item("padding", 0u8);
    Ok(())
}

fn fill_hcpe_output(
    out: &Bound<'_, PyAny>,
    data: &[u8; 38],
    score: i16,
    best_move16: u16,
    game_result: i8,
) -> PyResult<()> {
    if let Ok(len) = out.len()
        && len == 38
    {
        for (idx, byte) in data.iter().copied().enumerate() {
            out.set_item(idx, byte)?;
        }
        return Ok(());
    }

    if let Ok(first) = out.get_item(0)
        && fill_hcpe_fields(&first, data, score, best_move16, game_result).is_ok()
    {
        return Ok(());
    }

    if fill_hcpe_fields(out, data, score, best_move16, game_result).is_ok() {
        return Ok(());
    }

    Err(PyTypeError::new_err(
        "out must be writable uint8[38] or HuffmanCodedPosAndEval ndarray with fields",
    ))
}

fn fill_hcpe_fields(
    target: &Bound<'_, PyAny>,
    data: &[u8; 38],
    score: i16,
    best_move16: u16,
    game_result: i8,
) -> PyResult<()> {
    let field = target.get_item("hcp")?;
    if field.len().ok() != Some(32) {
        return Err(PyTypeError::new_err("hcp field must have length 32"));
    }
    for (idx, byte) in data[..32].iter().copied().enumerate() {
        field.set_item(idx, byte)?;
    }
    target.set_item("eval", score)?;
    target.set_item("bestMove16", best_move16)?;
    target.set_item("gameResult", game_result)?;
    let _ = target.set_item("dummy", 0u8);
    Ok(())
}

fn collect_py_moves(list: &MoveList) -> Vec<PyMove> {
    list.iter().copied().map(PyMove::from_move).collect()
}

fn collect_py_moves32(position: &Position, list: &MoveList) -> Vec<PyMove32> {
    list.iter().copied().map(|mv| PyMove32 { inner: position.move32_from_move(mv) }).collect()
}

fn game_result_name(result: GameResult) -> &'static str {
    match result {
        GameResult::BlackWin => "BLACK_WIN",
        GameResult::WhiteWin => "WHITE_WIN",
        GameResult::DrawByRepetition => "DRAW_BY_REPETITION",
        GameResult::Error => "ERROR",
        GameResult::BlackWinByDeclaration => "BLACK_WIN_BY_DECLARATION",
        GameResult::WhiteWinByDeclaration => "WHITE_WIN_BY_DECLARATION",
        GameResult::DrawByMaxPlies => "DRAW_BY_MAX_PLIES",
        GameResult::Invalid => "INVALID",
        GameResult::BlackWinByForfeit => "BLACK_WIN_BY_FORFEIT",
        GameResult::WhiteWinByForfeit => "WHITE_WIN_BY_FORFEIT",
        GameResult::DrawByImpasse => "DRAW_BY_IMPASSE",
        GameResult::BlackWinByTryRule => "BLACK_WIN_BY_TRY_RULE",
        GameResult::WhiteWinByTryRule => "WHITE_WIN_BY_TRY_RULE",
        GameResult::BlackWinByIllegalMove => "BLACK_WIN_BY_ILLEGAL_MOVE",
        GameResult::WhiteWinByIllegalMove => "WHITE_WIN_BY_ILLEGAL_MOVE",
        GameResult::Paused => "PAUSED",
        GameResult::BlackWinByTimeout => "BLACK_WIN_BY_TIMEOUT",
        GameResult::WhiteWinByTimeout => "WHITE_WIN_BY_TIMEOUT",
    }
}

fn game_result_from_str(name: &str) -> Option<GameResult> {
    match name.to_ascii_uppercase().as_str() {
        "BLACK_WIN" => Some(GameResult::BlackWin),
        "WHITE_WIN" => Some(GameResult::WhiteWin),
        "DRAW_BY_REPETITION" => Some(GameResult::DrawByRepetition),
        "ERROR" => Some(GameResult::Error),
        "BLACK_WIN_BY_DECLARATION" => Some(GameResult::BlackWinByDeclaration),
        "WHITE_WIN_BY_DECLARATION" => Some(GameResult::WhiteWinByDeclaration),
        "DRAW_BY_MAX_PLIES" => Some(GameResult::DrawByMaxPlies),
        "INVALID" => Some(GameResult::Invalid),
        "BLACK_WIN_BY_FORFEIT" => Some(GameResult::BlackWinByForfeit),
        "WHITE_WIN_BY_FORFEIT" => Some(GameResult::WhiteWinByForfeit),
        "DRAW_BY_IMPASSE" => Some(GameResult::DrawByImpasse),
        "BLACK_WIN_BY_TRY_RULE" => Some(GameResult::BlackWinByTryRule),
        "WHITE_WIN_BY_TRY_RULE" => Some(GameResult::WhiteWinByTryRule),
        "BLACK_WIN_BY_ILLEGAL_MOVE" => Some(GameResult::BlackWinByIllegalMove),
        "WHITE_WIN_BY_ILLEGAL_MOVE" => Some(GameResult::WhiteWinByIllegalMove),
        "PAUSED" => Some(GameResult::Paused),
        "BLACK_WIN_BY_TIMEOUT" => Some(GameResult::BlackWinByTimeout),
        "WHITE_WIN_BY_TIMEOUT" => Some(GameResult::WhiteWinByTimeout),
        _ => None,
    }
}

fn game_result_from_value(value: u8) -> Option<GameResult> {
    match value {
        0 => Some(GameResult::BlackWin),
        1 => Some(GameResult::WhiteWin),
        2 => Some(GameResult::DrawByRepetition),
        3 => Some(GameResult::Error),
        4 => Some(GameResult::BlackWinByDeclaration),
        5 => Some(GameResult::WhiteWinByDeclaration),
        6 => Some(GameResult::DrawByMaxPlies),
        7 => Some(GameResult::Invalid),
        8 => Some(GameResult::BlackWinByForfeit),
        9 => Some(GameResult::WhiteWinByForfeit),
        10 => Some(GameResult::DrawByImpasse),
        11 => Some(GameResult::Paused),
        12 => Some(GameResult::BlackWinByIllegalMove),
        13 => Some(GameResult::WhiteWinByIllegalMove),
        16 => Some(GameResult::BlackWinByTimeout),
        17 => Some(GameResult::WhiteWinByTimeout),
        20 => Some(GameResult::BlackWinByTryRule),
        21 => Some(GameResult::WhiteWinByTryRule),
        _ => None,
    }
}

fn special_move_from_game_result(result: GameResult) -> SpecialMove {
    match result {
        GameResult::BlackWin | GameResult::WhiteWin => SpecialMove::Resign,
        GameResult::DrawByRepetition => SpecialMove::RepetitionDraw,
        GameResult::Error | GameResult::Invalid | GameResult::Paused => SpecialMove::Interrupt,
        GameResult::BlackWinByDeclaration | GameResult::WhiteWinByDeclaration => {
            SpecialMove::WinByDeclaration
        }
        GameResult::BlackWinByTryRule | GameResult::WhiteWinByTryRule => SpecialMove::Try,
        GameResult::DrawByMaxPlies => SpecialMove::MaxMoves,
        GameResult::BlackWinByForfeit | GameResult::WhiteWinByForfeit => SpecialMove::WinByDefault,
        GameResult::DrawByImpasse => SpecialMove::Impasse,
        GameResult::BlackWinByIllegalMove | GameResult::WhiteWinByIllegalMove => {
            SpecialMove::WinByIllegalMove
        }
        GameResult::BlackWinByTimeout | GameResult::WhiteWinByTimeout => SpecialMove::Timeout,
    }
}

fn special_move_name(kind: &SpecialMove) -> &'static str {
    match kind {
        SpecialMove::Interrupt => "INTERRUPT",
        SpecialMove::Resign => "RESIGN",
        SpecialMove::MaxMoves => "MAX_MOVES",
        SpecialMove::Impasse => "IMPASSE",
        SpecialMove::Draw => "DRAW",
        SpecialMove::RepetitionDraw => "REPETITION_DRAW",
        SpecialMove::Mate => "MATE",
        SpecialMove::NoMate => "NO_MATE",
        SpecialMove::Timeout => "TIMEOUT",
        SpecialMove::WinByIllegalMove => "WIN_BY_ILLEGAL_MOVE",
        SpecialMove::LoseByIllegalMove => "LOSE_BY_ILLEGAL_MOVE",
        SpecialMove::WinByDeclaration => "WIN_BY_DECLARATION",
        SpecialMove::WinByDefault => "WIN_BY_DEFAULT",
        SpecialMove::LoseByDefault => "LOSE_BY_DEFAULT",
        SpecialMove::Try => "TRY",
        SpecialMove::Unknown(_) => "UNKNOWN",
    }
}

#[cfg(all(test, feature = "python-tests"))]
mod tests {
    use super::*;
    use ::rsshogi::board::movegen::{Evasions, NonEvasions, generate_moves};

    const STARTPOS_SFEN: &str = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";
    const DOUBLE_CHECK_SFEN: &str =
        "ln2+r1r2/5s+Pkl/3+B1p1p1/p4B2p/2P6/P6PP/1PNP1P3/2G3SK1/L4G1NL w 2GSN3Ps3p 76";

    #[test]
    fn py_move_from_usi_rejects_special_moves() {
        Python::attach(|py| {
            let cls = py.get_type::<PyMove>();
            assert!(PyMove::from_usi(&cls, "7g7f").expect("normal move").is_normal());
            assert!(PyMove::from_usi(&cls, "none").is_err());
            assert!(PyMove::from_usi(&cls, "null").is_err());
        });
    }

    #[test]
    fn pseudo_legal_moves_in_check_uses_evasions() {
        let board = PyBoard::new(Some(DOUBLE_CHECK_SFEN)).expect("board init");
        assert!(board.position.is_in_check(), "position should be in check for this test");

        let mut expected = MoveList::new();
        generate_moves::<Evasions>(&board.position, &mut expected);
        let actual = board.pseudo_legal_moves();

        assert_eq!(actual.len(), expected.len());
    }

    #[test]
    fn pseudo_legal_moves_not_in_check_uses_non_evasions() {
        let board = PyBoard::new(Some(STARTPOS_SFEN)).expect("board init");
        assert!(!board.position.is_in_check(), "position should not be in check");

        let mut expected = MoveList::new();
        generate_moves::<NonEvasions>(&board.position, &mut expected);
        let actual = board.pseudo_legal_moves();

        assert_eq!(actual.len(), expected.len());
    }

    #[test]
    fn in_check_and_is_mated_match_core_semantics() {
        let board = PyBoard::new(Some(STARTPOS_SFEN)).expect("board init");
        assert_eq!(board.is_in_check(), board.position.is_in_check());
        assert_eq!(board.is_mated(), board.position.is_mated());

        let board = PyBoard::new(Some(DOUBLE_CHECK_SFEN)).expect("board init");
        assert_eq!(board.is_in_check(), board.position.is_in_check());
        assert_eq!(board.is_mated(), board.position.is_mated());
    }

    #[test]
    fn repetition_helpers_default_to_none() {
        let board = PyBoard::new(Some(STARTPOS_SFEN)).expect("board init");
        assert_eq!(board.position.repetition_state(), ::rsshogi::types::RepetitionState::None);
        assert!(!board.is_repetition(3));
    }

    #[test]
    fn set_usi_position_accepts_startpos_with_moves() {
        let mut board = PyBoard::new(None).expect("board init");
        board.set_usi_position("startpos moves 7g7f 3c3d").expect("position should parse");
        assert_eq!(
            board.to_sfen(),
            "lnsgkgsnl/1r5b1/pppppp1pp/6p2/9/2P6/PP1PPPPPP/1B5R1/LNSGKGSNL b - 3"
        );
    }

    #[test]
    fn parse_usi_position_rejects_illegal_moves() {
        let result = parse_usi_position("startpos moves 7g7f 7g7f");
        assert!(result.is_err());
    }

    #[test]
    fn normalize_usi_position_returns_startpos_without_moves() {
        let normalized = normalize_usi_position("position startpos").expect("normalize");
        assert_eq!(normalized, "startpos");
    }

    #[test]
    fn normalize_usi_position_returns_canonical_sfen_with_moves() {
        let normalized = normalize_usi_position("position startpos moves 7g7f").expect("normalize");
        assert_eq!(normalized, "lnsgkgsnl/1r5b1/ppppppppp/9/9/2P6/PP1PPPPPP/1B5R1/LNSGKGSNL w - 2");
    }

    #[test]
    fn game_result_class_attrs_match_values() {
        Python::attach(|py| {
            let cls = py.get_type::<PyGameResult>();
            let black = cls
                .getattr("BLACK_WIN")
                .expect("class attr")
                .extract::<PyRef<'_, PyGameResult>>()
                .expect("game result");
            assert_eq!(black.value(), GameResult::BlackWin as u8);
        });
    }

    #[test]
    fn parse_kif_exposes_record_result() {
        let kif = "\
手合割：平手
手数----指手---------消費時間--
 1 ７六歩(77)
 2 投了
まで1手で先手の勝ち";
        let inner = parse_kif_str_record(kif).expect("parse");
        let record = PyRecord { inner };
        assert_eq!(record.result().value(), GameResult::BlackWin as u8);
        assert_eq!(record.move_count(), 1);
    }

    #[test]
    fn parse_kif_exposes_main_terminal() {
        let kif = "\
手合割：平手
手数----指手---------消費時間--
 1 ７六歩(77)
 2 投了
まで1手で先手の勝ち";
        let inner = parse_kif_str_record(kif).expect("parse");
        let record = PyRecord { inner };
        let terminal = record.main_terminal().expect("terminal");
        assert_eq!(terminal.kind(), "RESIGN");
        assert_eq!(terminal.result().value(), GameResult::BlackWin as u8);
        let raw = terminal.raw().expect("raw");
        assert!(raw.contains("投了"));
    }

    #[test]
    fn parse_csa_exposes_terminal_time_and_comment() {
        let csa = "\
V2.2
N+先手
N-後手
PI
+
+7776FU
T3
-3334FU
T4
%TORYO
T5
'*終局コメント";
        let inner = parse_csa_str_record(csa).expect("parse");
        let record = PyRecord { inner };
        let terminal = record.main_terminal().expect("terminal");
        assert_eq!(terminal.kind(), "RESIGN");
        assert_eq!(terminal.result().value(), GameResult::WhiteWin as u8);
        assert_eq!(terminal.time_ms(), Some(5000));
        assert_eq!(terminal.comment().as_deref(), Some("終局コメント"));
        assert_eq!(terminal.raw().as_deref(), Some("%TORYO"));
    }

    #[test]
    fn record_node_accessors_reject_oversized_node_id_without_panic() {
        let record = PyRecord { inner: Record::new(STARTPOS_SFEN).expect("record") };
        assert!(record.node_parent(usize::MAX).is_err());
        assert!(record.node_move(usize::MAX).is_err());

        let mut editor = PyRecordEditor::new("startpos").expect("editor");
        assert!(editor.go_to(usize::MAX).is_err());
    }

    #[test]
    fn book_key_from_position_works() {
        let board = PyBoard::new(Some(STARTPOS_SFEN)).expect("board init");
        let key = book_key_from_position(&board);
        assert_ne!(key.low(), 0);
    }

    #[test]
    fn book_key_after_matches_push() {
        Python::attach(|py| {
            let mut board = PyBoard::new(Some(STARTPOS_SFEN)).expect("board init");
            let mv = PyMove::from_move(Move::from_usi("7g7f").expect("valid move"));

            let key_after = book_key_after(&board, &mv);

            let _py_mv = Py::new(py, mv).expect("py wrap");
            board.apply_usi("7g7f").expect("legal move");
            let key_current = book_key_from_position(&board);

            assert_eq!(key_after.low(), key_current.low());
            assert_eq!(key_after.high(), key_current.high());
        });
    }

    #[test]
    fn memory_book_insert_and_get() {
        let mut book = PyMemoryBook::new();
        let board = PyBoard::new(Some(STARTPOS_SFEN)).expect("board init");
        let key = book_key_from_position(&board);

        let mv = PyMove::from_move(Move::from_usi("7g7f").expect("valid move"));
        let book_move = PyBookMove::from_book_move(BookMove::new(mv.inner, 100, 10));

        book.insert_move(&key, &book_move);

        let entry = book.get(&key).expect("entry exists");
        assert_eq!(entry.moves().len(), 1);
        assert_eq!(entry.moves()[0].score(), 100);
    }

    #[test]
    fn static_book_roundtrip() {
        let mut memory_book = PyMemoryBook::new();
        let board = PyBoard::new(Some(STARTPOS_SFEN)).expect("board init");
        let key = book_key_from_position(&board);

        let mv = PyMove::from_move(Move::from_usi("7g7f").expect("valid move"));
        let book_move = PyBookMove::from_book_move(BookMove::new(mv.inner, 100, 10));
        memory_book.insert_move(&key, &book_move);

        Python::attach(|py| {
            let cls = py.get_type::<PyStaticBook>();
            let static_book = PyStaticBook::from_memory_book(&cls, &memory_book);
            let bytes = static_book.to_bytes(py);

            let restored =
                PyStaticBook::from_bytes(&cls, bytes.as_any()).expect("roundtrip succeeds");

            assert_eq!(restored.inner.len(), 1);
            let entry = restored.get(&key).expect("entry exists");
            assert_eq!(entry.moves().len(), 1);
        });
    }
}
