use pyo3::basic::CompareOp;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyType;
use std::convert::TryFrom;

use ::rsshogi::types::moves::MoveType;
use ::rsshogi::types::{
    Bitboard, Color, Hand, HandPiece, Piece, PieceType, RepetitionState, Square,
};

fn piece_type_to_hand_piece(pt: PieceType) -> PyResult<HandPiece> {
    HandPiece::from_piece_type(pt).ok_or_else(|| {
        PyValueError::new_err(format!("piece type {} cannot be converted to a hand piece", pt))
    })
}

fn compare_bool(op: CompareOp, equal: bool) -> PyResult<bool> {
    match op {
        CompareOp::Eq => Ok(equal),
        CompareOp::Ne => Ok(!equal),
        _ => Err(PyTypeError::new_err("comparison only supports == and !=")),
    }
}

#[pyclass(name = "Color")]
#[derive(Clone, Copy)]
pub struct PyColor {
    inner: Color,
}

#[pymethods]
#[allow(non_snake_case)]
impl PyColor {
    #[new]
    fn new(value: Option<i8>) -> PyResult<Self> {
        let value = value.unwrap_or(Color::BLACK.raw());
        let color =
            Color::from_i8(value).ok_or_else(|| PyValueError::new_err("invalid color value"))?;
        Ok(Self { inner: color })
    }

    #[classattr]
    fn BLACK(py: Python<'_>) -> PyResult<Py<PyColor>> {
        Py::new(py, PyColor { inner: Color::BLACK })
    }

    #[classattr]
    fn WHITE(py: Python<'_>) -> PyResult<Py<PyColor>> {
        Py::new(py, PyColor { inner: Color::WHITE })
    }

    #[classmethod]
    fn from_usi(_cls: &Bound<'_, PyType>, usi: &str) -> PyResult<Self> {
        let color = match usi.to_ascii_lowercase().as_str() {
            "b" => Color::BLACK,
            "w" => Color::WHITE,
            _ => usi.parse::<Color>().map_err(PyValueError::new_err)?,
        };
        Ok(Self { inner: color })
    }

    fn flip(&self) -> Self {
        Self { inner: self.inner.flip() }
    }

    fn opponent(&self) -> Self {
        self.flip()
    }

    fn is_black(&self) -> bool {
        self.inner == Color::BLACK
    }

    fn is_white(&self) -> bool {
        self.inner == Color::WHITE
    }

    #[getter]
    fn value(&self) -> i8 {
        self.inner.raw()
    }

    fn __int__(&self) -> i8 {
        self.value()
    }

    fn __hash__(&self) -> i64 {
        i64::from(self.inner.raw())
    }

    fn __repr__(&self) -> String {
        format!("Color({})", self.inner)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        let other_value = match other.extract::<PyRef<'_, PyColor>>() {
            Ok(other) => other.inner.raw(),
            _ => match other.extract::<i8>() {
                Ok(value) => value,
                _ => {
                    return Err(PyTypeError::new_err("unsupported comparison for Color"));
                }
            },
        };
        compare_bool(op, self.inner.raw() == other_value)
    }
}

#[pyclass(name = "PieceType")]
#[derive(Clone, Copy)]
pub struct PyPieceType {
    inner: PieceType,
}

#[pymethods]
#[allow(non_snake_case)]
impl PyPieceType {
    #[new]
    fn new(value: Option<i8>) -> PyResult<Self> {
        let value = value.unwrap_or(PieceType::NONE.raw());
        let piece_type = PieceType::new(value);
        if !piece_type.is_valid() {
            return Err(PyValueError::new_err("invalid piece type value"));
        }
        Ok(Self { inner: piece_type })
    }

    #[classattr]
    fn NO_PIECE_TYPE(py: Python<'_>) -> PyResult<Py<PyPieceType>> {
        Py::new(py, PyPieceType { inner: PieceType::NONE })
    }

    #[classattr]
    fn PAWN(py: Python<'_>) -> PyResult<Py<PyPieceType>> {
        Py::new(py, PyPieceType { inner: PieceType::PAWN })
    }

    #[classattr]
    fn LANCE(py: Python<'_>) -> PyResult<Py<PyPieceType>> {
        Py::new(py, PyPieceType { inner: PieceType::LANCE })
    }

    #[classattr]
    fn KNIGHT(py: Python<'_>) -> PyResult<Py<PyPieceType>> {
        Py::new(py, PyPieceType { inner: PieceType::KNIGHT })
    }

    #[classattr]
    fn SILVER(py: Python<'_>) -> PyResult<Py<PyPieceType>> {
        Py::new(py, PyPieceType { inner: PieceType::SILVER })
    }

    #[classattr]
    fn BISHOP(py: Python<'_>) -> PyResult<Py<PyPieceType>> {
        Py::new(py, PyPieceType { inner: PieceType::BISHOP })
    }

    #[classattr]
    fn ROOK(py: Python<'_>) -> PyResult<Py<PyPieceType>> {
        Py::new(py, PyPieceType { inner: PieceType::ROOK })
    }

    #[classattr]
    fn GOLD(py: Python<'_>) -> PyResult<Py<PyPieceType>> {
        Py::new(py, PyPieceType { inner: PieceType::GOLD })
    }

    #[classattr]
    fn KING(py: Python<'_>) -> PyResult<Py<PyPieceType>> {
        Py::new(py, PyPieceType { inner: PieceType::KING })
    }

    #[classattr]
    fn PRO_PAWN(py: Python<'_>) -> PyResult<Py<PyPieceType>> {
        Py::new(py, PyPieceType { inner: PieceType::PRO_PAWN })
    }

    #[classattr]
    fn PRO_LANCE(py: Python<'_>) -> PyResult<Py<PyPieceType>> {
        Py::new(py, PyPieceType { inner: PieceType::PRO_LANCE })
    }

    #[classattr]
    fn PRO_KNIGHT(py: Python<'_>) -> PyResult<Py<PyPieceType>> {
        Py::new(py, PyPieceType { inner: PieceType::PRO_KNIGHT })
    }

    #[classattr]
    fn PRO_SILVER(py: Python<'_>) -> PyResult<Py<PyPieceType>> {
        Py::new(py, PyPieceType { inner: PieceType::PRO_SILVER })
    }

    #[classattr]
    fn HORSE(py: Python<'_>) -> PyResult<Py<PyPieceType>> {
        Py::new(py, PyPieceType { inner: PieceType::HORSE })
    }

    #[classattr]
    fn DRAGON(py: Python<'_>) -> PyResult<Py<PyPieceType>> {
        Py::new(py, PyPieceType { inner: PieceType::DRAGON })
    }

    #[classattr]
    fn GOLD_LIKE(py: Python<'_>) -> PyResult<Py<PyPieceType>> {
        Py::new(py, PyPieceType { inner: PieceType::GOLD_LIKE })
    }

    #[classmethod]
    fn from_usi(_cls: &Bound<'_, PyType>, usi: &str) -> PyResult<Self> {
        let piece_type = usi.parse::<PieceType>().map_err(PyValueError::new_err)?;
        Ok(Self { inner: piece_type })
    }

    fn is_promotable(&self) -> bool {
        self.inner.is_promotable()
    }

    fn is_promoted(&self) -> bool {
        self.inner.is_promoted()
    }

    fn promote(&self) -> Self {
        Self { inner: self.inner.promote() }
    }

    fn demote(&self) -> Self {
        Self { inner: self.inner.demote() }
    }

    #[getter]
    fn value(&self) -> i8 {
        self.inner.raw()
    }

    #[allow(clippy::wrong_self_convention)]
    fn to_usi(&self) -> String {
        self.inner.to_string()
    }

    fn __int__(&self) -> i8 {
        self.value()
    }

    fn __hash__(&self) -> i64 {
        i64::from(self.inner.raw())
    }

    fn __repr__(&self) -> String {
        format!("PieceType({})", self.inner)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        let other_value = match other.extract::<PyRef<'_, PyPieceType>>() {
            Ok(other) => other.inner.raw(),
            _ => match other.extract::<i8>() {
                Ok(value) => value,
                _ => {
                    return Err(PyTypeError::new_err("unsupported comparison for PieceType"));
                }
            },
        };
        compare_bool(op, self.inner.raw() == other_value)
    }
}

#[pyclass(name = "Piece")]
#[derive(Clone, Copy)]
pub struct PyPiece {
    inner: Piece,
}

#[pymethods]
#[allow(dead_code)]
impl PyPiece {
    #[new]
    fn new(value: Option<i8>) -> PyResult<Self> {
        let value = value.unwrap_or(Piece::NONE.raw());
        let piece = Piece::new(value);
        if !piece.is_valid() {
            return Err(PyValueError::new_err("invalid piece value"));
        }
        Ok(Self { inner: piece })
    }

    #[classmethod]
    fn from_color_type(
        _cls: &Bound<'_, PyType>,
        color: &PyColor,
        piece_type: &PyPieceType,
    ) -> Self {
        Self { inner: Piece::from_parts(color.inner, piece_type.inner) }
    }

    #[classmethod]
    fn from_usi(_cls: &Bound<'_, PyType>, usi: &str) -> PyResult<Self> {
        let piece = usi.parse::<Piece>().map_err(PyValueError::new_err)?;
        Ok(Self { inner: piece })
    }

    #[getter]
    fn color(&self) -> PyColor {
        PyColor { inner: self.inner.color() }
    }

    #[getter]
    fn piece_type(&self) -> PyPieceType {
        PyPieceType { inner: self.inner.piece_type() }
    }

    fn is_promoted(&self) -> bool {
        self.inner.is_promoted()
    }

    fn promote(&self) -> Self {
        Self { inner: self.inner.promote() }
    }

    fn unpromoted_piece(&self) -> Self {
        Self { inner: self.inner.unpromoted_piece() }
    }

    fn base_piece_type(&self) -> PyPieceType {
        PyPieceType { inner: self.inner.base_piece_type() }
    }

    #[getter]
    fn value(&self) -> i8 {
        self.inner.raw()
    }

    fn __int__(&self) -> i8 {
        self.value()
    }

    fn __hash__(&self) -> i64 {
        i64::from(self.inner.raw())
    }

    fn __repr__(&self) -> String {
        format!("Piece({})", self.inner)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        let other_value = match other.extract::<PyRef<'_, PyPiece>>() {
            Ok(other) => other.inner.raw(),
            _ => match other.extract::<i8>() {
                Ok(value) => value,
                _ => {
                    return Err(PyTypeError::new_err("unsupported comparison for Piece"));
                }
            },
        };
        compare_bool(op, self.inner.raw() == other_value)
    }
}

#[pyclass(name = "Square")]
#[derive(Clone, Copy)]
pub struct PySquare {
    inner: Square,
}

#[pymethods]
#[allow(non_snake_case)]
impl PySquare {
    #[new]
    fn new(value: Option<i8>) -> PyResult<Self> {
        let value = value.unwrap_or(Square::NONE.raw());
        let sq = Square::new(value);
        if !sq.is_valid() {
            return Err(PyValueError::new_err("invalid square value"));
        }
        Ok(Self { inner: sq })
    }

    #[classattr]
    fn NONE(py: Python<'_>) -> PyResult<Py<PySquare>> {
        Py::new(py, PySquare { inner: Square::NONE })
    }

    #[classmethod]
    fn from_usi(_cls: &Bound<'_, PyType>, usi: &str) -> PyResult<Self> {
        let sq = usi.parse::<Square>().map_err(PyValueError::new_err)?;
        Ok(Self { inner: sq })
    }

    #[classmethod]
    fn from_index(_cls: &Bound<'_, PyType>, idx: usize) -> PyResult<Self> {
        if idx >= Square::COUNT {
            return Err(PyValueError::new_err("square index out of range"));
        }
        Ok(Self {
            inner: Square::new(
                i8::try_from(idx).map_err(|_| PyValueError::new_err("index overflow"))?,
            ),
        })
    }

    #[classmethod]
    fn from_file_rank(_cls: &Bound<'_, PyType>, file: i8, rank: i8) -> PyResult<Self> {
        if !(0..=8).contains(&file) || !(0..=8).contains(&rank) {
            return Err(PyValueError::new_err("file and rank must be in 0..8"));
        }
        Ok(Self { inner: Square::new(file * 9 + rank) })
    }

    #[allow(clippy::wrong_self_convention)]
    fn to_usi(&self) -> String {
        self.inner.to_string()
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn is_none(&self) -> bool {
        self.inner.is_none()
    }

    fn mirror_file(&self) -> Self {
        Self { inner: self.inner.mirror_file() }
    }

    fn flip_side(&self) -> Self {
        Self { inner: self.inner.flip() }
    }

    #[getter]
    fn value(&self) -> i8 {
        self.inner.raw()
    }

    #[getter]
    fn file(&self) -> i8 {
        self.inner.file().raw()
    }

    #[getter]
    fn rank(&self) -> i8 {
        self.inner.rank().raw()
    }

    fn __int__(&self) -> i8 {
        self.value()
    }

    fn __hash__(&self) -> i64 {
        i64::from(self.inner.raw())
    }

    fn __repr__(&self) -> String {
        format!("Square({})", self.inner)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        let other_value = match other.extract::<PyRef<'_, PySquare>>() {
            Ok(other) => other.inner.raw(),
            _ => match other.extract::<i8>() {
                Ok(value) => value,
                _ => {
                    return Err(PyTypeError::new_err("unsupported comparison for Square"));
                }
            },
        };
        compare_bool(op, self.inner.raw() == other_value)
    }
}

#[pyclass(name = "Bitboard")]
#[derive(Clone)]
pub struct PyBitboard {
    inner: Bitboard,
}

#[pymethods]
#[allow(non_snake_case)]
impl PyBitboard {
    #[new]
    fn new() -> Self {
        Self { inner: Bitboard::EMPTY }
    }

    #[classattr]
    fn EMPTY(py: Python<'_>) -> PyResult<Py<PyBitboard>> {
        Py::new(py, PyBitboard { inner: Bitboard::EMPTY })
    }

    #[classattr]
    fn ALL(py: Python<'_>) -> PyResult<Py<PyBitboard>> {
        Py::new(py, PyBitboard { inner: Bitboard::ALL })
    }

    #[classmethod]
    fn from_parts(_cls: &Bound<'_, PyType>, low: u64, high: u64) -> Self {
        Self { inner: Bitboard::from_parts([low, high]) }
    }

    #[classmethod]
    fn from_square(_cls: &Bound<'_, PyType>, square: &PySquare) -> Self {
        Self { inner: Bitboard::from_square(square.inner) }
    }

    #[getter]
    fn packed(&self) -> u128 {
        self.inner.packed_bits()
    }

    #[getter]
    fn parts(&self) -> (u64, u64) {
        let parts = self.inner.parts();
        (parts[0], parts[1])
    }

    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn count(&self) -> u32 {
        self.inner.count()
    }

    fn set(&mut self, square: &PySquare) {
        self.inner.set(square.inner)
    }

    fn clear(&mut self, square: &PySquare) {
        self.inner.clear(square.inner)
    }

    fn test(&self, square: &PySquare) -> bool {
        self.inner.test(square.inner)
    }

    fn flip_side(&self) -> Self {
        Self { inner: self.inner.flip() }
    }

    fn mirror_file(&self) -> Self {
        Self { inner: self.inner.mirror_file() }
    }

    fn pop_lsb(&mut self) -> Option<PySquare> {
        self.inner.pop_lsb().map(|sq| PySquare { inner: sq })
    }

    fn __int__(&self) -> u128 {
        self.inner.packed_bits()
    }

    fn __repr__(&self) -> String {
        format!("Bitboard(0x{:x})", self.inner.packed_bits())
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }
}

#[pyclass(name = "Hand")]
#[derive(Clone, Copy)]
pub struct PyHand {
    inner: Hand,
}

#[pymethods]
#[allow(dead_code)]
#[allow(non_snake_case)]
impl PyHand {
    #[new]
    fn new() -> Self {
        Self { inner: Hand::ZERO }
    }

    #[classattr]
    fn ZERO(py: Python<'_>) -> PyResult<Py<PyHand>> {
        Py::new(py, PyHand { inner: Hand::ZERO })
    }

    fn count(&self, piece_type: &PyPieceType) -> PyResult<u32> {
        let hp = piece_type_to_hand_piece(piece_type.inner)?;
        Ok(self.inner.count(hp))
    }

    fn add(&mut self, piece_type: &PyPieceType, amount: Option<u32>) -> PyResult<()> {
        let hp = piece_type_to_hand_piece(piece_type.inner)?;
        let amount = amount.unwrap_or(1);
        self.inner.add(hp, amount);
        Ok(())
    }

    fn sub(&mut self, piece_type: &PyPieceType, amount: Option<u32>) -> PyResult<()> {
        let hp = piece_type_to_hand_piece(piece_type.inner)?;
        let amount = amount.unwrap_or(1);
        self.inner.sub(hp, amount);
        Ok(())
    }

    fn is_empty(&self) -> bool {
        self.inner.bits() == 0
    }

    #[getter]
    fn value(&self) -> u32 {
        self.inner.bits()
    }

    fn __int__(&self) -> u32 {
        self.value()
    }

    fn __repr__(&self) -> String {
        format!("Hand(0x{:x})", self.inner.bits())
    }

    fn __str__(&self) -> String {
        format!("0x{:x}", self.inner.bits())
    }
}

#[pyclass(name = "MoveType")]
#[derive(Clone, Copy)]
pub struct PyMoveType {
    inner: MoveType,
}

#[pymethods]
#[allow(dead_code)]
#[allow(non_snake_case)]
impl PyMoveType {
    #[new]
    fn new() -> Self {
        Self { inner: MoveType::Normal }
    }

    #[classattr]
    fn NORMAL(py: Python<'_>) -> PyResult<Py<PyMoveType>> {
        Py::new(py, PyMoveType { inner: MoveType::Normal })
    }

    #[classattr]
    fn PROMOTION(py: Python<'_>) -> PyResult<Py<PyMoveType>> {
        Py::new(py, PyMoveType { inner: MoveType::Promotion })
    }

    #[classattr]
    fn DROP(py: Python<'_>) -> PyResult<Py<PyMoveType>> {
        Py::new(py, PyMoveType { inner: MoveType::Drop })
    }

    fn is_normal(&self) -> bool {
        matches!(self.inner, MoveType::Normal)
    }

    fn is_promotion(&self) -> bool {
        matches!(self.inner, MoveType::Promotion)
    }

    fn is_drop(&self) -> bool {
        matches!(self.inner, MoveType::Drop)
    }

    fn __repr__(&self) -> &'static str {
        match self.inner {
            MoveType::Normal => "MoveType.NORMAL",
            MoveType::Promotion => "MoveType.PROMOTION",
            MoveType::Drop => "MoveType.DROP",
        }
    }

    fn __str__(&self) -> &'static str {
        match self.inner {
            MoveType::Normal => "normal",
            MoveType::Promotion => "promotion",
            MoveType::Drop => "drop",
        }
    }

    fn __hash__(&self) -> i64 {
        let value = match self.inner {
            MoveType::Normal => 0,
            MoveType::Promotion => 1,
            MoveType::Drop => 2,
        };
        i64::from(value)
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        let other_value = match other.extract::<PyRef<'_, PyMoveType>>() {
            Ok(other) => other.inner,
            _ => {
                return Err(PyTypeError::new_err("unsupported comparison for MoveType"));
            }
        };
        compare_bool(op, self.inner == other_value)
    }
}

#[pyclass(name = "RepetitionState")]
#[derive(Clone, Copy)]
pub struct PyRepetitionState {
    inner: RepetitionState,
}

#[pymethods]
#[allow(dead_code)]
#[allow(non_snake_case)]
impl PyRepetitionState {
    #[new]
    fn new() -> Self {
        Self { inner: RepetitionState::None }
    }

    #[classattr]
    fn NONE(py: Python<'_>) -> PyResult<Py<PyRepetitionState>> {
        Py::new(py, PyRepetitionState { inner: RepetitionState::None })
    }

    #[classattr]
    fn WIN(py: Python<'_>) -> PyResult<Py<PyRepetitionState>> {
        Py::new(py, PyRepetitionState { inner: RepetitionState::Win })
    }

    #[classattr]
    fn LOSE(py: Python<'_>) -> PyResult<Py<PyRepetitionState>> {
        Py::new(py, PyRepetitionState { inner: RepetitionState::Lose })
    }

    #[classattr]
    fn DRAW(py: Python<'_>) -> PyResult<Py<PyRepetitionState>> {
        Py::new(py, PyRepetitionState { inner: RepetitionState::Draw })
    }

    #[classattr]
    fn SUPERIOR(py: Python<'_>) -> PyResult<Py<PyRepetitionState>> {
        Py::new(py, PyRepetitionState { inner: RepetitionState::Superior })
    }

    #[classattr]
    fn INFERIOR(py: Python<'_>) -> PyResult<Py<PyRepetitionState>> {
        Py::new(py, PyRepetitionState { inner: RepetitionState::Inferior })
    }

    #[allow(clippy::wrong_self_convention)]
    fn to_usi(&self) -> &'static str {
        self.inner.to_usi()
    }

    fn __repr__(&self) -> String {
        format!("RepetitionState({})", self.inner.to_usi())
    }

    fn __str__(&self) -> &'static str {
        self.inner.to_usi()
    }

    fn __hash__(&self) -> i64 {
        let value = match self.inner {
            RepetitionState::None => 0,
            RepetitionState::Win => 1,
            RepetitionState::Lose => 2,
            RepetitionState::Draw => 3,
            RepetitionState::Superior => 4,
            RepetitionState::Inferior => 5,
        };
        i64::from(value)
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        let other_value = match other.extract::<PyRef<'_, PyRepetitionState>>() {
            Ok(other) => other.inner,
            _ => {
                return Err(PyTypeError::new_err("unsupported comparison for RepetitionState"));
            }
        };
        compare_bool(op, self.inner == other_value)
    }
}

impl PyColor {
    pub(crate) fn from_color(inner: Color) -> Self {
        Self { inner }
    }

    #[allow(dead_code)]
    pub(crate) fn inner(&self) -> Color {
        self.inner
    }
}

impl PyPieceType {
    pub(crate) fn from_piece_type(inner: PieceType) -> Self {
        Self { inner }
    }

    #[allow(dead_code)]
    pub(crate) fn inner(&self) -> PieceType {
        self.inner
    }
}

impl PyPiece {
    pub(crate) fn from_piece(inner: Piece) -> Self {
        Self { inner }
    }

    #[allow(dead_code)]
    pub(crate) fn inner(&self) -> Piece {
        self.inner
    }
}

impl PySquare {
    pub(crate) fn from_square(inner: Square) -> Self {
        Self { inner }
    }

    #[allow(dead_code)]
    pub(crate) fn inner(&self) -> Square {
        self.inner
    }
}

impl PyBitboard {
    pub(crate) fn from_bitboard(inner: Bitboard) -> Self {
        Self { inner }
    }

    #[allow(dead_code)]
    pub(crate) fn inner(&self) -> Bitboard {
        self.inner
    }
}

impl PyHand {
    pub(crate) fn from_hand(inner: Hand) -> Self {
        Self { inner }
    }

    #[allow(dead_code)]
    pub(crate) fn inner(&self) -> Hand {
        self.inner
    }
}

impl PyMoveType {
    pub(crate) fn from_move_type(inner: MoveType) -> Self {
        Self { inner }
    }

    #[allow(dead_code)]
    pub(crate) fn inner(&self) -> MoveType {
        self.inner
    }
}

impl PyRepetitionState {
    pub(crate) fn from_state(inner: RepetitionState) -> Self {
        Self { inner }
    }

    #[allow(dead_code)]
    pub(crate) fn inner(&self) -> RepetitionState {
        self.inner
    }
}
