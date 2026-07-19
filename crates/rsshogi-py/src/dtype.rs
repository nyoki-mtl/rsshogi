use pyo3::exceptions::{PyAttributeError, PyImportError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict};
use std::ffi::CString;

type NumpyDtypes = (Py<PyAny>, Py<PyAny>, Py<PyAny>, Py<PyAny>);

pub(crate) fn add_dtype_module(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    let dtype_module = PyModule::new(py, "dtype")?;
    dtype_module.add_function(wrap_pyfunction!(dtype_getattr, dtype_module.clone())?)?;
    let getattr = dtype_module.getattr("dtype_getattr")?;
    dtype_module.setattr("__getattr__", getattr)?;

    if let Ok((packed_sfen, packed_sfen_value, hcp, hcpe)) = build_numpy_dtypes(py) {
        dtype_module.add("PackedSfen", packed_sfen)?;
        dtype_module.add("PackedSfenValue", packed_sfen_value)?;
        dtype_module.add("HuffmanCodedPos", hcp)?;
        dtype_module.add("HuffmanCodedPosAndEval", hcpe)?;
    }

    m.add_submodule(&dtype_module)?;
    Ok(())
}

fn build_numpy_dtypes(py: Python<'_>) -> PyResult<NumpyDtypes> {
    let numpy = py.import("numpy")?;
    let locals = PyDict::new(py);
    locals.set_item("np", &numpy)?;

    let packed_sfen_expr = CString::new("np.dtype([('sfen', np.uint8, 32)])")?;
    let packed_sfen = py.eval(packed_sfen_expr.as_c_str(), Some(&locals), None)?.unbind();

    let packed_sfen_value_expr = CString::new(
        "np.dtype([('sfen', np.uint8, 32), ('score', np.int16), ('move', np.uint16), ('game_ply', np.uint16), ('game_result', np.int8), ('padding', np.uint8)])",
    )?;
    let packed_sfen_value =
        py.eval(packed_sfen_value_expr.as_c_str(), Some(&locals), None)?.unbind();

    let hcp_expr = CString::new("np.dtype([('hcp', np.uint8, 32)])")?;
    let hcp = py.eval(hcp_expr.as_c_str(), Some(&locals), None)?.unbind();

    let hcpe_expr = CString::new(
        "np.dtype([('hcp', np.uint8, 32), ('eval', np.int16), ('bestMove16', np.uint16), ('gameResult', np.int8), ('dummy', np.uint8)])",
    )?;
    let hcpe = py.eval(hcpe_expr.as_c_str(), Some(&locals), None)?.unbind();

    Ok((packed_sfen, packed_sfen_value, hcp, hcpe))
}

#[pyfunction]
fn dtype_getattr(py: Python<'_>, name: &str) -> PyResult<Py<PyAny>> {
    let (packed_sfen, packed_sfen_value, hcp, hcpe) = build_numpy_dtypes(py).map_err(|_| {
        PyImportError::new_err(format!("numpy is required for rsshogi.numpy.{name}"))
    })?;

    match name {
        "PackedSfen" => Ok(packed_sfen),
        "PackedSfenValue" => Ok(packed_sfen_value),
        "HuffmanCodedPos" => Ok(hcp),
        "HuffmanCodedPosAndEval" => Ok(hcpe),
        _ => Err(PyAttributeError::new_err(format!(
            "module 'rsshogi.numpy' has no attribute '{name}'"
        ))),
    }
}
