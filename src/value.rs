//! The `Value` Python class to build WebAssembly values.

use crate::wasmer::runtime::Value as WasmValue;
use pyo3::{class::basic::PyObjectProtocol, prelude::*, PyNativeType};

#[pyclass(unsendable)]
/// The `Value` class represents a WebAssembly value.
pub struct Value {
    pub value: WasmValue,
}

unsafe impl PyNativeType for Value {}

#[pymethods]
impl Value {
    /// Build a WebAssembly `i32` value.
    #[staticmethod]
    #[text_signature = "(value)"]
    fn i32(value: i32) -> PyResult<Self> {
        Ok(Self {
            value: WasmValue::I32(value),
        })
    }

    /// Build a WebAssembly `i64` value.
    #[staticmethod]
    #[text_signature = "(value)"]
    fn i64(value: i64) -> PyResult<Self> {
        Ok(Self {
            value: WasmValue::I64(value),
        })
    }

    /// Build a WebAssembly `f32` value.
    #[staticmethod]
    #[text_signature = "(value)"]
    fn f32(value: f32) -> PyResult<Self> {
        Ok(Self {
            value: WasmValue::F32(value),
        })
    }

    /// Build a WebAssembly `f64` value.
    #[staticmethod]
    #[text_signature = "(value)"]
    fn f64(value: f64) -> PyResult<Self> {
        Ok(Self {
            value: WasmValue::F64(value),
        })
    }

    /// Build a WebAssembly `v128` value.
    #[staticmethod]
    #[text_signature = "(value)"]
    fn v128(value: u128) -> PyResult<Self> {
        Ok(Self {
            value: WasmValue::V128(value),
        })
    }
}

#[pyproto]
impl PyObjectProtocol for Value {
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("{:?}", self.value))
    }
}
