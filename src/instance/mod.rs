//! The `wasmer.Instance` Python object to build WebAssembly instances.
//!
//! The `Instance` class has the following declaration:
//!
//! * The constructor reads bytes from its first parameter, and it
//!   expects those bytes to represent a valid WebAssembly module,
//! * The `exports` getter, to get exported functions from the
//!   WebAssembly module, e.g. `instance.exports.sum(1, 2)` to call the
//!   exported function `sum` with arguments `1` and `2`,
//! * The `memory` getter, to get the exported memory (if any) from
//!   the WebAssembly module, .e.g. `instance.memory.uint8_view()`, see
//!   the `wasmer.Memory` class.

pub(crate) mod exports;
pub(crate) mod globals;
pub(crate) mod inspect;

use crate::{
    import::ImportObject,
    instance::exports::ExportedFunctions,
    instance::globals::ExportedGlobals,
    memory::Memory,
    wasmer::runtime::{self as runtime, Export},
};
use pyo3::{
    exceptions::RuntimeError,
    prelude::*,
    pycell::PyCell,
    types::{PyAny, PyBytes, PyDict},
    PyObject, Python,
};
use std::{collections::HashMap, sync::Arc};

#[pyclass(unsendable)]
#[text_signature = "(bytes, imported_functions={})"]
/// `Instance` is a Python class that represents a WebAssembly instance.
///
/// # Examples
///
/// ```python
/// from wasmer import Instance
///
/// instance = Instance(wasm_bytes)
/// ```
pub struct Instance {
    pub(crate) instance: Arc<runtime::Instance>,

    /// All WebAssembly exported functions represented by an
    /// `ExportedFunctions` object.
    pub(crate) exports: Py<ExportedFunctions>,

    /// The WebAssembly exported memory represented by a `Memory`
    /// object.
    pub(crate) memory: Option<Py<Memory>>,

    /// All WebAssembly exported globals represented by an
    /// `ExportedGlobals` object.
    pub(crate) globals: Py<ExportedGlobals>,

    exports_index_to_name: Option<HashMap<usize, String>>,
}

impl Instance {
    pub(crate) fn inner_new(
        instance: Arc<runtime::Instance>,
        exports: Py<ExportedFunctions>,
        memory: Option<Py<Memory>>,
        globals: Py<ExportedGlobals>,
    ) -> Self {
        Self {
            instance,
            exports,
            memory,
            globals,
            exports_index_to_name: None,
        }
    }
}

#[pymethods]
/// Implement methods on the `Instance` Python class.
impl Instance {
    /// The constructor instantiates a new WebAssembly instance based
    /// on WebAssembly bytes (represented by the Python bytes type).
    #[new]
    #[args(import_object = "PyDict::new(_py).as_ref()")]
    fn new(py: Python, bytes: &PyAny, import_object: &PyAny) -> PyResult<Self> {
        // Read the bytes.
        let bytes = bytes.downcast::<PyBytes>()?.as_bytes();

        // Compile the module.
        let module = runtime::compile(bytes).map_err(|error| {
            RuntimeError::py_err(format!("Failed to compile the module:\n    {}", error))
        })?;

        // Instantiate the WebAssembly module, with an import object.
        let instance = if let Ok(import_object) = import_object.downcast::<PyCell<ImportObject>>() {
            let import_object = import_object.borrow();

            module.instantiate(&(*import_object).inner)
        } else if let Ok(imported_functions) = import_object.downcast::<PyDict>() {
            let module = Arc::new(module);
            let mut import_object = ImportObject::new(module.clone());
            import_object.extend_with_pydict(py, imported_functions)?;

            module.instantiate(&import_object.inner)
        } else {
            return Err(RuntimeError::py_err(
                "The `imported_functions` parameter contains an unknown value. Python dictionaries or `wasmer.ImportObject` are the only supported values.".to_string()
            ));
        };

        let instance = instance.map(Arc::new).map_err(|e| {
            RuntimeError::py_err(format!("Failed to instantiate the module:\n    {}", e))
        })?;

        let exports = instance.exports();

        // Collect the exported functions, globals and memory from the
        // WebAssembly module.
        let mut exported_functions = Vec::new();
        let mut exported_globals = Vec::new();
        let mut exported_memory = None;

        for (export_name, export) in exports {
            match export {
                Export::Function { .. } => exported_functions.push(export_name.clone()),
                Export::Global(global) => {
                    exported_globals.push((export_name.clone(), Arc::new(global.into())))
                }
                Export::Memory(memory) if exported_memory.is_none() => {
                    exported_memory = Some(Arc::new(memory.into()))
                }
                _ => (),
            }
        }

        Ok(Self::inner_new(
            instance.clone(),
            Py::new(
                py,
                ExportedFunctions {
                    instance: instance.clone(),
                    functions: exported_functions,
                },
            )?,
            match exported_memory {
                Some(memory) => Some(Py::new(py, Memory { memory })?),
                None => None,
            },
            Py::new(
                py,
                ExportedGlobals {
                    globals: exported_globals,
                },
            )?,
        ))
    }

    /// The `exports` getter.
    #[getter]
    fn exports(&self) -> &Py<ExportedFunctions> {
        &self.exports
    }

    /// The `memory` getter.
    #[getter]
    fn memory(&self, py: Python) -> PyResult<PyObject> {
        match &self.memory {
            Some(memory) => Ok(memory.into_py(py)),
            None => Ok(py.None()),
        }
    }

    /// The `globals` getter.
    #[getter]
    fn globals(&self) -> &Py<ExportedGlobals> {
        &self.globals
    }

    /// Find the export _name_ associated to an index if it is valid.
    #[text_signature = "($self, index)"]
    fn resolve_exported_function(&mut self, py: Python, index: usize) -> PyResult<String> {
        match &self.exports_index_to_name {
            Some(exports_index_to_name) => {
                exports_index_to_name.get(&index).cloned().ok_or_else(|| {
                    RuntimeError::py_err(format!("Function at index `{}` does not exist.", index))
                })
            }

            None => {
                self.exports_index_to_name = Some(
                    self.instance
                        .exports()
                        .filter(|(_, export)| match export {
                            Export::Function { .. } => true,
                            _ => false,
                        })
                        .map(|(name, _)| (self.instance.resolve_func(&name).unwrap(), name.clone()))
                        .collect(),
                );

                self.resolve_exported_function(py, index)
            }
        }
    }
}
