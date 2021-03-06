use pyo3::prelude::*;

use wasmer_engines::OpaqueCompiler;

/// The Singlepass compiler, designed for the `wasmer` Python package
/// (a WebAssembly runtime).
///
/// Please check the documentation of `wasmer.engine` to learn more.
#[pymodule]
fn wasmer_compiler_singlepass(_py: Python, module: &PyModule) -> PyResult<()> {
    module.add_class::<Compiler>()?;

    Ok(())
}

/// The Singlepass compiler.
///
/// ## Example
///
/// ```py
/// from wasmer import engine, Store
/// from wasmer_compiler_singlepass import Compiler
///
/// store = Store(engine.JIT(Compiler))
/// ```
#[pyclass]
struct Compiler {}

#[pymethods]
impl Compiler {
    /// Please don't use it. Internal use only.
    #[staticmethod]
    fn into_opaque_compiler() -> OpaqueCompiler {
        OpaqueCompiler::raw_with_compiler(
            wasmer_compiler_singlepass::Singlepass::default(),
            "singlepass".to_string(),
        )
    }
}
