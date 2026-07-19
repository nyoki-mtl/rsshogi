//! SAZ2自己対局教師データのPython binding。

use std::fs;

use pyo3::{
    exceptions::PyValueError,
    prelude::*,
    types::{PyAny, PyBytes, PyList},
};

use ::rsshogi::{
    board::Position,
    records::formats::sazpack::{
        SazOutcomeBound, SazSelfplayGame, SazSelfplayPolicyEntry, SazSelfplayPosition,
        SazTerminationReason, SazWdl, deserialize_selfplay_chunk, serialize_selfplay_chunk,
    },
    types::EnteringKingRule,
};

use crate::{PyGameResult, parse_game_result_arg, parse_move_arg, path_to_string};

fn value_error(error: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(format!("sazpack: {error}"))
}

#[pyclass(name = "SazWdl")]
#[derive(Clone)]
pub(crate) struct PySazWdl {
    inner: SazWdl,
}

#[pymethods]
impl PySazWdl {
    #[new]
    fn new(win: u16, draw: u16, loss: u16) -> Self {
        Self { inner: SazWdl { win, draw, loss } }
    }

    #[getter]
    fn win(&self) -> u16 {
        self.inner.win
    }
    #[getter]
    fn draw(&self) -> u16 {
        self.inner.draw
    }
    #[getter]
    fn loss(&self) -> u16 {
        self.inner.loss
    }
}

#[pyclass(name = "SazPolicyEntry")]
#[derive(Clone)]
pub(crate) struct PySazPolicyEntry {
    inner: SazSelfplayPolicyEntry,
}

#[pymethods]
impl PySazPolicyEntry {
    #[new]
    fn new(
        mv: &Bound<'_, PyAny>,
        prior: u16,
        visits_before: u32,
        visits_after: u32,
        lower: u8,
        upper: u8,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: SazSelfplayPolicyEntry {
                mv: parse_move_arg(mv)?,
                prior,
                visits_before,
                visits_after,
                lower: SazOutcomeBound::try_from(lower).map_err(value_error)?,
                upper: SazOutcomeBound::try_from(upper).map_err(value_error)?,
            },
        })
    }

    #[getter]
    fn mv(&self) -> String {
        self.inner.mv.to_usi()
    }
    #[getter]
    fn prior(&self) -> u16 {
        self.inner.prior
    }
    #[getter]
    fn visits_before(&self) -> u32 {
        self.inner.visits_before
    }
    #[getter]
    fn visits_after(&self) -> u32 {
        self.inner.visits_after
    }
    #[getter]
    fn lower(&self) -> u8 {
        self.inner.lower as u8
    }
    #[getter]
    fn upper(&self) -> u8 {
        self.inner.upper as u8
    }
}

#[pyclass(name = "SazPosition")]
#[derive(Clone)]
pub(crate) struct PySazPosition {
    inner: SazSelfplayPosition,
}

#[pymethods]
impl PySazPosition {
    #[new]
    #[pyo3(signature = (played, root_wdl, outcome_wdl, plies_left, requested_visits, target_weight_milli, exploration_flags, policy, mate=None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        played: &Bound<'_, PyAny>,
        root_wdl: PySazWdl,
        outcome_wdl: PySazWdl,
        plies_left: u16,
        requested_visits: u32,
        target_weight_milli: u16,
        exploration_flags: u8,
        policy: Vec<PySazPolicyEntry>,
        mate: Option<i16>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: SazSelfplayPosition {
                played: parse_move_arg(played)?,
                root_wdl: root_wdl.inner,
                outcome_wdl: outcome_wdl.inner,
                plies_left,
                requested_visits,
                target_weight_milli,
                exploration_flags,
                mate,
                policy: policy.into_iter().map(|entry| entry.inner).collect(),
            },
        })
    }

    #[getter]
    fn played(&self) -> String {
        self.inner.played.to_usi()
    }
    #[getter]
    fn root_wdl(&self) -> PySazWdl {
        PySazWdl { inner: self.inner.root_wdl }
    }
    #[getter]
    fn outcome_wdl(&self) -> PySazWdl {
        PySazWdl { inner: self.inner.outcome_wdl }
    }
    #[getter]
    fn plies_left(&self) -> u16 {
        self.inner.plies_left
    }
    #[getter]
    fn requested_visits(&self) -> u32 {
        self.inner.requested_visits
    }
    #[getter]
    fn target_weight_milli(&self) -> u16 {
        self.inner.target_weight_milli
    }
    #[getter]
    fn exploration_flags(&self) -> u8 {
        self.inner.exploration_flags
    }
    #[getter]
    fn mate(&self) -> Option<i16> {
        self.inner.mate
    }
    #[getter]
    fn policy(&self) -> Vec<PySazPolicyEntry> {
        self.inner.policy.iter().cloned().map(|inner| PySazPolicyEntry { inner }).collect()
    }
}

#[pyclass(name = "SazGame")]
#[derive(Clone)]
pub(crate) struct PySazGame {
    inner: SazSelfplayGame,
}

#[pymethods]
impl PySazGame {
    #[new]
    fn new(
        stem: &Bound<'_, PyAny>,
        game_result: &Bound<'_, PyAny>,
        termination_reason: u8,
        entering_king_rule: u8,
        positions: Vec<PySazPosition>,
    ) -> PyResult<Self> {
        ::rsshogi::board::init();
        let stem_packed_sfen = if let Ok(sfen) = stem.extract::<String>() {
            let mut position = Position::empty();
            position.set_sfen(&sfen).map_err(value_error)?;
            position.to_packed_sfen()
        } else {
            let bytes = stem.extract::<Vec<u8>>()?;
            let data: [u8; 32] =
                bytes.try_into().map_err(|_| value_error("stem packed sfen must be 32 bytes"))?;
            ::rsshogi::board::PackedSfen::from_bytes(data)
        };
        Ok(Self {
            inner: SazSelfplayGame {
                stem_packed_sfen,
                game_result: parse_game_result_arg(game_result)?,
                termination_reason: SazTerminationReason::try_from(termination_reason)
                    .map_err(value_error)?,
                entering_king_rule: entering_king_rule_from_u8(entering_king_rule)?,
                positions: positions.into_iter().map(|position| position.inner).collect(),
            },
        })
    }

    #[getter]
    fn stem_packed_sfen<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.stem_packed_sfen.data)
    }
    #[getter]
    fn game_result(&self) -> PyGameResult {
        PyGameResult { inner: self.inner.game_result }
    }
    #[getter]
    fn termination_reason(&self) -> u8 {
        self.inner.termination_reason as u8
    }
    #[getter]
    fn entering_king_rule(&self) -> u8 {
        entering_king_rule_to_u8(self.inner.entering_king_rule)
    }
    #[getter]
    fn positions(&self) -> Vec<PySazPosition> {
        self.inner.positions.iter().cloned().map(|inner| PySazPosition { inner }).collect()
    }
}

fn extract_games(games: &Bound<'_, PyAny>) -> PyResult<Vec<SazSelfplayGame>> {
    games
        .try_iter()?
        .map(|item| {
            let item = item?;
            let game = item
                .extract::<PyRef<'_, PySazGame>>()
                .map_err(|_| value_error("games must contain SazGame instances"))?;
            Ok(game.inner.clone())
        })
        .collect()
}

fn games_to_list<'py>(
    py: Python<'py>,
    games: Vec<SazSelfplayGame>,
) -> PyResult<Bound<'py, PyList>> {
    let list = PyList::empty(py);
    for inner in games {
        list.append(PySazGame { inner })?;
    }
    Ok(list)
}

#[pyfunction]
pub(crate) fn write_sazpack<'py>(
    py: Python<'py>,
    games: &Bound<'_, PyAny>,
) -> PyResult<Bound<'py, PyBytes>> {
    let bytes = serialize_selfplay_chunk(&extract_games(games)?).map_err(value_error)?;
    Ok(PyBytes::new(py, &bytes))
}

#[pyfunction]
pub(crate) fn write_sazpack_file(
    path: &Bound<'_, PyAny>,
    games: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let path = path_to_string(path)?;
    let bytes = serialize_selfplay_chunk(&extract_games(games)?).map_err(value_error)?;
    fs::write(&path, bytes).map_err(value_error)
}

#[pyfunction]
pub(crate) fn decode_sazpack<'py>(
    py: Python<'py>,
    data: &Bound<'_, PyAny>,
) -> PyResult<Bound<'py, PyList>> {
    games_to_list(py, deserialize_selfplay_chunk(&data.extract::<Vec<u8>>()?).map_err(value_error)?)
}

#[pyfunction]
pub(crate) fn decode_sazpack_file<'py>(
    py: Python<'py>,
    path: &Bound<'_, PyAny>,
) -> PyResult<Bound<'py, PyList>> {
    let path = path_to_string(path)?;
    games_to_list(
        py,
        deserialize_selfplay_chunk(&fs::read(path).map_err(value_error)?).map_err(value_error)?,
    )
}

const fn entering_king_rule_to_u8(rule: EnteringKingRule) -> u8 {
    match rule {
        EnteringKingRule::None => 0,
        EnteringKingRule::Point24 => 1,
        EnteringKingRule::Point24Handicap => 2,
        EnteringKingRule::Point27 => 3,
        EnteringKingRule::Point27Handicap => 4,
        EnteringKingRule::TryRule => 5,
        EnteringKingRule::Unset => 6,
    }
}

fn entering_king_rule_from_u8(value: u8) -> PyResult<EnteringKingRule> {
    match value {
        0 => Ok(EnteringKingRule::None),
        1 => Ok(EnteringKingRule::Point24),
        2 => Ok(EnteringKingRule::Point24Handicap),
        3 => Ok(EnteringKingRule::Point27),
        4 => Ok(EnteringKingRule::Point27Handicap),
        5 => Ok(EnteringKingRule::TryRule),
        6 => Ok(EnteringKingRule::Unset),
        _ => Err(value_error("invalid entering king rule")),
    }
}

pub(crate) fn register_sazpack(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PySazWdl>()?;
    module.add_class::<PySazPolicyEntry>()?;
    module.add_class::<PySazPosition>()?;
    module.add_class::<PySazGame>()?;
    module.add_function(wrap_pyfunction!(write_sazpack, module)?)?;
    module.add_function(wrap_pyfunction!(write_sazpack_file, module)?)?;
    module.add_function(wrap_pyfunction!(decode_sazpack, module)?)?;
    module.add_function(wrap_pyfunction!(decode_sazpack_file, module)?)?;
    Ok(())
}
