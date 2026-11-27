use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

/// Routes a KiCad board. `request_json` is a `copper_kicad::RouteRequest`; `progress`, when
/// given, is called with each progress update as JSON and returns True to stop routing.
#[pyfunction]
#[pyo3(signature = (request_json, progress=None))]
fn route_json(
    py: Python<'_>,
    request_json: String,
    progress: Option<PyObject>,
) -> PyResult<String> {
    py.allow_threads(move || {
        let mut on_progress = |update: &copper_kicad::Progress| -> bool {
            let Some(callback) = progress.as_ref() else {
                return false;
            };
            let Ok(text) = serde_json_text(update) else {
                return false;
            };
            Python::with_gil(|py| {
                callback
                    .call1(py, (text,))
                    .and_then(|stop| stop.is_truthy(py))
                    .unwrap_or_else(|error| {
                        error.print(py);
                        true
                    })
            })
        };
        copper_kicad::route_json(&request_json, &mut on_progress)
    })
    .map_err(PyRuntimeError::new_err)
}

fn serde_json_text(progress: &copper_kicad::Progress) -> Result<String, ()> {
    copper_kicad::progress_json(progress).map_err(|_| ())
}

#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(route_json, module)?)?;
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
