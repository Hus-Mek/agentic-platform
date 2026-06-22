//! Deprecated shim. The real WASM sandbox now lives in [`crate::wasm_sandbox`]
//! (`run_wasm` + `SandboxLimits`). This thin wrapper is kept only so older
//! call sites referencing `WasmSandbox` keep compiling.

use serde_json::Value;

use crate::wasm_sandbox::{run_wasm, SandboxLimits};

#[deprecated(note = "use crate::wasm_sandbox::run_wasm")]
pub struct WasmSandbox;

#[allow(deprecated)]
impl WasmSandbox {
    pub fn new() -> Self {
        Self
    }

    /// Execute a guest module with default limits and empty JSON input.
    pub fn execute(&self, wasm_bytes: &[u8], _func_name: &str) -> anyhow::Result<String> {
        let out = run_wasm(wasm_bytes, &Value::Null, &SandboxLimits::default())?;
        Ok(out.to_string())
    }
}

#[allow(deprecated)]
impl Default for WasmSandbox {
    fn default() -> Self {
        Self::new()
    }
}
