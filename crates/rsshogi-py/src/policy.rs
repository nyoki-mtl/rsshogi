use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyAny;

use ::rsshogi::labels::policy::{
    COMPACT_TO_MOVE_LABEL, CompactMoveLabel, MOVE_LABEL_TO_COMPACT, MoveLabel,
};

use crate::{PyColor, parse_move_arg};

pub(crate) fn register_policy_constants(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("MOVE_LABEL_COUNT", MoveLabel::COUNT)?;
    m.add("COMPACT_MOVE_LABEL_COUNT", CompactMoveLabel::COUNT)?;
    m.add("MOVE_LABEL_TO_COMPACT", MOVE_LABEL_TO_COMPACT.to_vec())?;
    m.add("COMPACT_TO_MOVE_LABEL", COMPACT_TO_MOVE_LABEL.to_vec())?;
    Ok(())
}

pub(crate) fn register_policy_functions(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(move_label, m)?)?;
    m.add_function(wrap_pyfunction!(compact_move_label, m)?)?;
    m.add_function(wrap_pyfunction!(move_label_to_compact, m)?)?;
    m.add_function(wrap_pyfunction!(compact_move_label_to_move_label, m)?)?;
    m.add_function(wrap_pyfunction!(is_structurally_valid_move_label, m)?)?;
    Ok(())
}

fn parse_move_label_raw(label: u16) -> PyResult<MoveLabel> {
    MoveLabel::from_raw(label).ok_or_else(|| PyValueError::new_err("move label must be in 0..2187"))
}

fn parse_compact_move_label_raw(label: u16) -> PyResult<CompactMoveLabel> {
    CompactMoveLabel::from_raw(label)
        .ok_or_else(|| PyValueError::new_err("compact move label must be in 0..1496"))
}

#[pyfunction]
fn move_label(r#move: &Bound<'_, PyAny>, turn: PyRef<'_, PyColor>) -> PyResult<u16> {
    let mv = parse_move_arg(r#move)?;
    let label = MoveLabel::from_move(mv, turn.inner())
        .ok_or_else(|| PyValueError::new_err("move cannot be encoded as a policy label"))?;
    Ok(label.raw())
}

#[pyfunction]
fn compact_move_label(
    r#move: &Bound<'_, PyAny>,
    turn: PyRef<'_, PyColor>,
) -> PyResult<Option<u16>> {
    let mv = parse_move_arg(r#move)?;
    Ok(CompactMoveLabel::from_move(mv, turn.inner()).map(CompactMoveLabel::raw))
}

#[pyfunction]
fn move_label_to_compact(label: u16) -> PyResult<Option<u16>> {
    Ok(parse_move_label_raw(label)?.compact().map(CompactMoveLabel::raw))
}

#[pyfunction]
fn compact_move_label_to_move_label(label: u16) -> PyResult<u16> {
    Ok(parse_compact_move_label_raw(label)?.expand().raw())
}

#[pyfunction]
fn is_structurally_valid_move_label(label: u16) -> PyResult<bool> {
    Ok(parse_move_label_raw(label)?.is_structurally_valid())
}
