//! The `wasmer.Module` Python object to build WebAssembly modules.

use crate::{
    instance::{
        exports::{ExportImportKind, ExportedFunctions},
        globals::ExportedGlobals,
        Instance,
    },
    memory::Memory,
};
use pyo3::{
    exceptions::RuntimeError,
    prelude::*,
    types::{PyAny, PyBytes, PyDict, PyList},
    PyTryFrom,
};
use std::rc::Rc;
use wasmer_runtime_core::{
    self as runtime_core,
    cache::Artifact,
    module::ExportIndex,
    types::{ExternDescriptor, ImportDescriptor, Type},
};
use wasmer_runtime_old::{self as runtime, imports, validate, Export};

#[pyclass]
#[text_signature = "(bytes)"]
/// `Module` is a Python class that represents a WebAssembly module.
pub struct Module {
    /// The underlying Rust WebAssembly module.
    module: runtime::Module,
}

#[pymethods]
/// Implement methods on the `Module` Python class.
impl Module {
    /// Compile bytes into a WebAssembly module.
    #[new]
    #[allow(clippy::new_ret_no_self)]
    fn new(bytes: &PyAny) -> PyResult<Self> {
        // Read the bytes.
        let bytes = <PyBytes as PyTryFrom>::try_from(bytes)?.as_bytes();

        // Compile the module.
        let module = runtime::compile(bytes).map_err(|error| {
            RuntimeError::py_err(format!("Failed to compile the module:\n    {}", error))
        })?;

        Ok(Self { module })
    }

    // Instantiate the module into an `Instance` Python object.
    #[text_signature = "($self)"]
    fn instantiate(&self, py: Python) -> PyResult<Py<Instance>> {
        let imports = imports! {};

        // Instantiate the module.
        let instance = match self.module.instantiate(&imports) {
            Ok(instance) => Rc::new(instance),
            Err(e) => {
                return Err(RuntimeError::py_err(format!(
                    "Failed to instantiate the module:\n    {}",
                    e
                )))
            }
        };

        let exports = instance.exports();

        // Collect the exported functions, globals and memory from the
        // WebAssembly module.
        let mut exported_functions = Vec::new();
        let mut exported_globals = Vec::new();
        let mut exported_memory = None;

        for (export_name, export) in exports.iter() {
            match export {
                Export::Function { .. } => exported_functions.push(export_name.clone()),
                Export::Global(global) => {
                    exported_globals.push((export_name.clone(), Rc::new(global.into())))
                }
                Export::Memory(memory) if exported_memory.is_none() => {
                    exported_memory = Some(Rc::new(memory.into()))
                }
                _ => (),
            }
        }

        // Instantiate the `Instance` Python class.
        Ok(Py::new(
            py,
            Instance::inner_new(
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
                Vec::new(),
            ),
        )?)
    }

    /// The `exports` getter returns all the exported functions as a
    /// list of dictionnaries with 2 pairs:
    ///
    ///   1. `"kind": <kind>`, where the kind is a `ExportKind` value.
    ///   2. `"name": <name>`, where the name is a string,
    #[getter]
    fn exports<'p>(&self, py: Python<'p>) -> PyResult<&'p PyList> {
        let exports = &self.module.info().exports;
        let mut items: Vec<&PyDict> = Vec::with_capacity(exports.len());

        for (name, export_index) in exports.iter() {
            let dict = PyDict::new(py);

            dict.set_item(
                "kind",
                match export_index {
                    ExportIndex::Function(_) => ExportImportKind::Function,
                    ExportIndex::Memory(_) => ExportImportKind::Memory,
                    ExportIndex::Global(_) => ExportImportKind::Global,
                    ExportIndex::Table(_) => ExportImportKind::Table,
                },
            )?;
            dict.set_item("name", name)?;

            items.push(dict);
        }

        Ok(PyList::new(py, items))
    }

    /// The `imports` getter returns all the imported functions as a
    /// list of dictionnaries with at least 3 pairs:
    ///
    ///   1. `"kind": <kind>`, where the kind is a `ImportKind` value.
    ///   2. `"namespace": <namespace>`, where the namespace is a string,
    ///   3. `"name": <name>`, where the name is a string.
    ///
    /// Additional pairs exist for the following kinds:
    ///
    ///   * `ImportKind.MEMORY` has the `"minimum_pages": {int}` and
    ///      `"maximum_pages": {int?}` pairs.
    ///   * `ImportKind.GLOBAL` has the `"mutable": {bool}` and
    ///     `"type": {string}` pairs.
    ///   * `ImportKind.TABLE` has the `"minimum_elements: {int}`,
    ///     `"maximum_elements: {int?}`, and `"element_type": {string}`
    ///     pairs.
    #[getter]
    fn imports<'p>(&self, py: Python<'p>) -> PyResult<&'p PyList> {
        let imports = self.module.imports();
        let mut items: Vec<&PyDict> = Vec::with_capacity(imports.len());

        for import_descriptor in imports {
            let dict = PyDict::new(py);
            let module = import_descriptor.module();
            let name = import_descriptor.name();
            let ty = import_descriptor.ty();

            match ty {
                ExternDescriptor::Function(_) => {
                    dict.set_item("kind", ExportImportKind::Function as u8)?;
                    dict.set_item("namespace", module)?;
                    dict.set_item("name", name)?;
                }

                ExternDescriptor::Memory(memory) => {
                    dict.set_item("kind", ExportImportKind::Memory as u8)?;
                    dict.set_item("namespace", module)?;
                    dict.set_item("name", name)?;
                    dict.set_item("minimum_pages", memory.minimum.0)?;
                    dict.set_item(
                        "maximum_pages",
                        memory
                            .maximum
                            .map(|page| page.0.into_py(py))
                            .unwrap_or_else(|| py.None()),
                    )?;
                }

                ExternDescriptor::Global(global) => {
                    let mutable: bool = global.mutability.into();

                    dict.set_item("kind", ExportImportKind::Global as u8)?;
                    dict.set_item("namespace", module)?;
                    dict.set_item("name", name)?;
                    dict.set_item("mutable", mutable)?;
                    dict.set_item(
                        "type",
                        match global.ty {
                            Type::I32 => "i32",
                            Type::I64 => "i64",
                            Type::F32 => "f32",
                            Type::F64 => "f64",
                            Type::V128 => "v128",
                            _ => unimplemented!("{}", global.ty),
                        },
                    )?;
                }

                ExternDescriptor::Table(table) => {
                    dict.set_item("kind", ExportImportKind::Table as u8)?;
                    dict.set_item("namespace", module)?;
                    dict.set_item("name", name)?;
                    dict.set_item("minimum_elements", table.minimum)?;
                    dict.set_item(
                        "maximum_elements",
                        table
                            .maximum
                            .map(|number| number.into_py(py))
                            .unwrap_or_else(|| py.None()),
                    )?;
                    dict.set_item(
                        "element_type",
                        match table.ty {
                            Type::I32 => "i32",
                            Type::I64 => "i64",
                            Type::F32 => "f32",
                            Type::F64 => "f64",
                            Type::V128 => "v128",
                            _ => unimplemented!("{}", table.ty),
                        },
                    )?;
                }
            }

            items.push(dict);
        }

        Ok(PyList::new(py, items))
    }

    /// Read all the custom section names. To get the value of a
    /// custom section, use the `Module.custom_section()`
    /// function. This designed is motivated by saving memory.
    #[getter]
    fn custom_section_names<'p>(&self, py: Python<'p>) -> &'p PyList {
        PyList::new(py, self.module.info().custom_sections.keys())
    }

    /// Read a specific custom section.
    #[text_signature = "($self, name, index=0)"]
    #[args(index = "0")]
    fn custom_section<'p>(&self, py: Python<'p>, name: String, index: usize) -> PyObject {
        match self.module.custom_sections(&name) {
            Some(bytes) => match bytes.get(index) {
                Some(bytes) => PyBytes::new(py, bytes).into_py(py),
                None => py.None(),
            },
            None => py.None(),
        }
    }

    /// Serialize the module into Python bytes.
    #[text_signature = "($self)"]
    fn serialize<'p>(&self, py: Python<'p>) -> PyResult<&'p PyBytes> {
        // Get the module artifact.
        match self.module.cache() {
            // Serialize the artifact.
            Ok(artifact) => match artifact.serialize() {
                Ok(serialized_artifact) => Ok(PyBytes::new(py, serialized_artifact.as_slice())),
                Err(_) => Err(RuntimeError::py_err(
                    "Failed to serialize the module artifact.",
                )),
            },
            Err(_) => Err(RuntimeError::py_err("Failed to get the module artifact.")),
        }
    }

    /// Deserialize Python bytes into a module instance.
    #[staticmethod]
    #[text_signature = "(bytes)"]
    fn deserialize(bytes: &PyAny, py: Python) -> PyResult<Py<Module>> {
        // Read the bytes.
        let serialized_module = <PyBytes as PyTryFrom>::try_from(bytes)?.as_bytes();

        // Deserialize the artifact.
        match unsafe { Artifact::deserialize(serialized_module) } {
            Ok(artifact) => {
                // Get the module from the artifact.
                match runtime::load_cache_with(artifact) {
                    Ok(module) => Ok(Py::new(py, Self { module })?),
                    Err(_) => Err(RuntimeError::py_err(
                        "Failed to compile the serialized module.",
                    )),
                }
            }
            Err(_) => Err(RuntimeError::py_err("Failed to deserialize the module.")),
        }
    }

    /// Check that given bytes represent a valid WebAssembly module.
    #[staticmethod]
    #[text_signature = "(bytes)"]
    fn validate(bytes: &PyAny) -> PyResult<bool> {
        match <PyBytes as PyTryFrom>::try_from(bytes) {
            Ok(bytes) => Ok(validate(bytes.as_bytes())),
            _ => Ok(false),
        }
    }
}
