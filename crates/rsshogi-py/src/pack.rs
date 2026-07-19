use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes, PyList};
use std::fs;
use std::fs::File;
use std::io::{BufWriter, Write};

use ::rsshogi::records::formats::sbinpack::{
    SbinpackChain, SbinpackMetadata,
    chains_from_record_with_metadata as chains_from_record_sbinpack,
    deserialize_file as deserialize_sbinpack_file, serialize_file as serialize_sbinpack_file,
};
use ::rsshogi::records::formats::{
    games_from_records as games_from_records_pack, records_from_bytes as records_from_bytes_pack,
    write_games as write_games_pack,
};
use ::rsshogi::records::record::Record;

use crate::{PyRecord, path_to_string, sbinpack_chain_to_record};

fn extract_game_records_sequence(records: &Bound<'_, PyAny>) -> PyResult<Vec<Record>> {
    let iter = records.try_iter()?;
    let mut out = Vec::new();
    for item in iter {
        let item = item?;
        let record = item
            .extract::<PyRef<'_, PyRecord>>()
            .map_err(|_| PyTypeError::new_err("records must contain Record instances"))?;
        out.push(record.inner.clone());
    }
    Ok(out)
}

fn collect_sbinpack_chains(
    records: &[Record],
    stem_score: i16,
    include_main: bool,
    include_variations: bool,
    metadatas: Option<Vec<Option<Vec<u8>>>>,
) -> PyResult<Vec<SbinpackChain>> {
    let metadatas = if let Some(metadatas) = metadatas {
        if metadatas.len() != records.len() {
            return Err(PyValueError::new_err(
                "sbinpack: metadatas length must match records length",
            ));
        }
        metadatas
    } else {
        vec![None; records.len()]
    };

    let mut chains = Vec::new();
    for (record, metadata) in records.iter().zip(metadatas) {
        let metadata = SbinpackMetadata::new(metadata.unwrap_or_default())
            .map_err(|err| PyValueError::new_err(format!("sbinpack: {err:?}")))?;
        let mut record_chains = chains_from_record_sbinpack(
            record,
            stem_score,
            metadata,
            include_main,
            include_variations,
        )
        .map_err(|err| PyValueError::new_err(format!("sbinpack: {err:?}")))?;
        chains.append(&mut record_chains);
    }

    if chains.is_empty() {
        return Err(PyValueError::new_err("sbinpack: no chains to serialize"));
    }
    Ok(chains)
}

fn sbinpack_chains_to_pylist<'py>(
    py: Python<'py>,
    chains: Vec<SbinpackChain>,
) -> PyResult<Bound<'py, PyList>> {
    let out = PyList::empty(py);
    for chain in chains {
        let metadata = PyBytes::new(py, chain.metadata.as_bytes());
        let record = sbinpack_chain_to_record(&chain)
            .map_err(|err| PyValueError::new_err(format!("sbinpack: {err}")))?;
        out.append((PyRecord { inner: record }, metadata))?;
    }
    Ok(out)
}

/// Decode every ``Record`` packed in a pack byte buffer.
///
/// This is the multi-record counterpart to ``Record.from_pack``, which
/// decodes a single record. Returns a list of records.
#[pyfunction]
pub(crate) fn decode_pack<'py>(
    py: Python<'py>,
    data: &Bound<'_, PyAny>,
) -> PyResult<Bound<'py, PyList>> {
    let bytes: Vec<u8> = data.extract()?;
    let records = records_from_bytes_pack(&bytes)
        .map_err(|err| PyValueError::new_err(format!("pack: {err}")))?;
    let out = PyList::empty(py);
    for record in records {
        out.append(PyRecord { inner: record })?;
    }
    Ok(out)
}

/// Decode every ``Record`` from a pack file on disk.
///
/// Multi-record counterpart to ``Record.from_pack`` (single record).
#[pyfunction]
pub(crate) fn decode_pack_file<'py>(
    py: Python<'py>,
    path: &Bound<'_, PyAny>,
) -> PyResult<Bound<'py, PyList>> {
    let path = path_to_string(path)?;
    let bytes = fs::read(&path).map_err(|err| {
        PyValueError::new_err(format!("failed to read pack file '{path}': {err}"))
    })?;
    let out = PyList::empty(py);
    for record in records_from_bytes_pack(&bytes)
        .map_err(|err| PyValueError::new_err(format!("pack: {err}")))?
    {
        out.append(PyRecord { inner: record })?;
    }
    Ok(out)
}

/// Decode every sbinpack chain in a byte buffer.
///
/// Returns ``(Record, metadata)`` tuples. This is the multi-chain
/// counterpart to ``Record.from_sbinpack_with_metadata``.
#[pyfunction]
pub(crate) fn decode_sbinpack<'py>(
    py: Python<'py>,
    data: &Bound<'_, PyAny>,
) -> PyResult<Bound<'py, PyList>> {
    let bytes: Vec<u8> = data.extract()?;
    let file = deserialize_sbinpack_file(&bytes)
        .map_err(|err| PyValueError::new_err(format!("sbinpack: {err:?}")))?;
    sbinpack_chains_to_pylist(py, file.chains)
}

/// Decode every sbinpack chain from a file on disk.
///
/// Returns ``(Record, metadata)`` tuples.
#[pyfunction]
pub(crate) fn decode_sbinpack_file<'py>(
    py: Python<'py>,
    path: &Bound<'_, PyAny>,
) -> PyResult<Bound<'py, PyList>> {
    let path = path_to_string(path)?;
    let bytes = fs::read(&path).map_err(|err| {
        PyValueError::new_err(format!("failed to read sbinpack file '{path}': {err}"))
    })?;
    let file = deserialize_sbinpack_file(&bytes)
        .map_err(|err| PyValueError::new_err(format!("sbinpack: {err:?}")))?;
    sbinpack_chains_to_pylist(py, file.chains)
}

#[pyfunction]
pub(crate) fn write_pack<'py>(
    py: Python<'py>,
    records: &Bound<'_, PyAny>,
) -> PyResult<Bound<'py, PyBytes>> {
    let records = extract_game_records_sequence(records)?;
    let games = games_from_records_pack(&records)
        .map_err(|err| PyValueError::new_err(format!("pack: {err}")))?;
    let mut bytes = Vec::new();
    write_games_pack(&mut bytes, &games)
        .map_err(|err| PyValueError::new_err(format!("pack: {err}")))?;
    Ok(PyBytes::new(py, &bytes))
}

#[pyfunction]
pub(crate) fn write_pack_file(path: &Bound<'_, PyAny>, records: &Bound<'_, PyAny>) -> PyResult<()> {
    let path = path_to_string(path)?;
    let records = extract_game_records_sequence(records)?;
    let games = games_from_records_pack(&records)
        .map_err(|err| PyValueError::new_err(format!("pack: {err}")))?;
    let file = File::create(&path).map_err(|err| {
        PyValueError::new_err(format!("failed to write pack file '{path}': {err}"))
    })?;
    let mut writer = BufWriter::new(file);
    write_games_pack(&mut writer, &games)
        .map_err(|err| PyValueError::new_err(format!("pack: {err}")))?;
    writer
        .flush()
        .map_err(|err| PyValueError::new_err(format!("failed to flush pack file '{path}': {err}")))
}

#[pyfunction]
#[pyo3(signature = (
    records,
    *,
    stem_score=0,
    include_main=true,
    include_variations=false,
    metadatas=None
))]
pub(crate) fn write_sbinpack<'py>(
    py: Python<'py>,
    records: &Bound<'_, PyAny>,
    stem_score: i16,
    include_main: bool,
    include_variations: bool,
    metadatas: Option<Vec<Option<Vec<u8>>>>,
) -> PyResult<Bound<'py, PyBytes>> {
    let records = extract_game_records_sequence(records)?;
    let chains =
        collect_sbinpack_chains(&records, stem_score, include_main, include_variations, metadatas)?;
    let bytes = serialize_sbinpack_file(&chains)
        .map_err(|err| PyValueError::new_err(format!("sbinpack: {err:?}")))?;
    Ok(PyBytes::new(py, &bytes))
}

#[pyfunction]
#[pyo3(signature = (
    path,
    records,
    *,
    stem_score=0,
    include_main=true,
    include_variations=false,
    metadatas=None
))]
pub(crate) fn write_sbinpack_file(
    path: &Bound<'_, PyAny>,
    records: &Bound<'_, PyAny>,
    stem_score: i16,
    include_main: bool,
    include_variations: bool,
    metadatas: Option<Vec<Option<Vec<u8>>>>,
) -> PyResult<()> {
    let path = path_to_string(path)?;
    let records = extract_game_records_sequence(records)?;
    let chains =
        collect_sbinpack_chains(&records, stem_score, include_main, include_variations, metadatas)?;
    let bytes = serialize_sbinpack_file(&chains)
        .map_err(|err| PyValueError::new_err(format!("sbinpack: {err:?}")))?;
    fs::write(&path, bytes).map_err(|err| {
        PyValueError::new_err(format!("failed to write sbinpack file '{path}': {err}"))
    })
}
