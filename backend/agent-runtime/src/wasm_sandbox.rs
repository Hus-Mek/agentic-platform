//! Real WASM code sandbox built on wasmtime.
//!
//! Executes untrusted guest modules with hard isolation: fuel-metered CPU,
//! a memory-size cap, and an epoch (wall-clock) deadline. No WASI, no host
//! imports, no network — the guest has zero ambient capabilities. This is the
//! safe execution primitive behind workflow `Script` nodes.
//!
//! ## Guest ABI ("memio" v1)
//! A guest module must export linear memory as `memory` and a function
//! `run` with signature `() -> ()`. The host exchanges JSON over memory:
//! - Input:  `mem[0..4]` = little-endian `u32` length, `mem[4..4+len]` = bytes.
//! - Output: `mem[OUT_BASE..OUT_BASE+4]` = LE `u32` length, then bytes.
//!
//! `OUT_BASE` is fixed at 1 MiB, so a conforming module must reserve at least
//! 17 pages of memory. This keeps the ABI trivial enough to target from WAT,
//! Rust→wasm, AssemblyScript, or a JS-on-wasm engine.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use serde_json::Value;
use wasmtime::{Config, Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder};

/// Base offset where the guest writes its output (1 MiB).
const OUT_BASE: usize = 0x0010_0000;
/// Hard ceiling on guest output size (4 MiB) regardless of declared length.
const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

/// Resource limits applied to a single sandboxed execution.
#[derive(Debug, Clone)]
pub struct SandboxLimits {
    /// Fuel units (roughly one per wasm instruction). Bounds CPU work.
    pub fuel: u64,
    /// Maximum linear-memory size in bytes.
    pub memory_bytes: usize,
    /// Wall-clock deadline in milliseconds (epoch-based interruption).
    pub wall_ms: u64,
}

impl Default for SandboxLimits {
    fn default() -> Self {
        Self {
            fuel: 200_000_000,
            memory_bytes: 64 * 1024 * 1024,
            wall_ms: 5_000,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SandboxError {
    #[error("failed to compile wasm module: {0}")]
    Compile(String),
    #[error("failed to instantiate wasm module: {0}")]
    Instantiate(String),
    #[error("guest module is missing required export `{0}`")]
    MissingExport(&'static str),
    #[error("guest execution trapped: {0}")]
    Trap(String),
    #[error("guest memory access out of bounds: {0}")]
    Memory(String),
    #[error("guest output length {0} exceeds limit {1}")]
    OutputTooLarge(usize, usize),
    #[error("input serialization failed: {0}")]
    Input(String),
    #[error("guest output was not valid JSON: {0}")]
    Output(String),
    #[error("sandbox engine error: {0}")]
    Engine(String),
}

struct SandboxState {
    limits: StoreLimits,
}

/// Run `input` through a guest wasm module under `limits`, returning the
/// guest's JSON output. The guest cannot touch the host filesystem, network,
/// clock, or any syscall — only the bytes the host hands it.
pub fn run_wasm(
    module_bytes: &[u8],
    input: &Value,
    limits: &SandboxLimits,
) -> Result<Value, SandboxError> {
    let input_bytes =
        serde_json::to_vec(input).map_err(|e| SandboxError::Input(e.to_string()))?;

    let mut config = Config::new();
    config.consume_fuel(true);
    config.epoch_interruption(true);
    let engine = Engine::new(&config).map_err(|e| SandboxError::Engine(e.to_string()))?;

    // `Module::new` accepts both binary wasm and WAT text bytes.
    let module =
        Module::new(&engine, module_bytes).map_err(|e| SandboxError::Compile(e.to_string()))?;

    let store_limits = StoreLimitsBuilder::new()
        .memory_size(limits.memory_bytes)
        .build();
    let mut store = Store::new(&engine, SandboxState { limits: store_limits });
    store.limiter(|s| &mut s.limits);
    store
        .set_fuel(limits.fuel)
        .map_err(|e| SandboxError::Engine(e.to_string()))?;
    store.set_epoch_deadline(1);

    // Empty linker => the module may import nothing. A module that declares any
    // import fails to instantiate, which is exactly the isolation guarantee.
    let linker = Linker::new(&engine);
    let instance = linker
        .instantiate(&mut store, &module)
        .map_err(|e| SandboxError::Instantiate(e.to_string()))?;

    let memory = instance
        .get_memory(&mut store, "memory")
        .ok_or(SandboxError::MissingExport("memory"))?;
    let run = instance
        .get_typed_func::<(), ()>(&mut store, "run")
        .map_err(|_| SandboxError::MissingExport("run"))?;

    // Write the length-prefixed input at offset 0.
    let len_le = (input_bytes.len() as u32).to_le_bytes();
    memory
        .write(&mut store, 0, &len_le)
        .map_err(|e| SandboxError::Memory(e.to_string()))?;
    memory
        .write(&mut store, 4, &input_bytes)
        .map_err(|e| SandboxError::Memory(e.to_string()))?;

    // Wall-clock watchdog: bump the engine epoch after `wall_ms` unless the
    // call finishes first (signalled over the channel). Cleanly cancellable so
    // a short call never leaves a lingering thread.
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let watchdog_engine = engine.clone();
    let wall_ms = limits.wall_ms;
    let watchdog = thread::spawn(move || {
        if done_rx.recv_timeout(Duration::from_millis(wall_ms)).is_err() {
            watchdog_engine.increment_epoch();
        }
    });

    let call_result = run.call(&mut store, ());
    let _ = done_tx.send(());
    let _ = watchdog.join();

    call_result.map_err(|e| SandboxError::Trap(e.to_string()))?;

    // Read the length-prefixed output from OUT_BASE.
    let mut out_len_le = [0u8; 4];
    memory
        .read(&mut store, OUT_BASE, &mut out_len_le)
        .map_err(|e| SandboxError::Memory(e.to_string()))?;
    let out_len = u32::from_le_bytes(out_len_le) as usize;
    if out_len > MAX_OUTPUT_BYTES {
        return Err(SandboxError::OutputTooLarge(out_len, MAX_OUTPUT_BYTES));
    }

    let mut out_bytes = vec![0u8; out_len];
    memory
        .read(&mut store, OUT_BASE + 4, &mut out_bytes)
        .map_err(|e| SandboxError::Memory(e.to_string()))?;

    serde_json::from_slice(&out_bytes).map_err(|e| SandboxError::Output(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// WAT module that echoes the JSON input straight back to the output region.
    const ECHO_WAT: &str = r#"
    (module
      (memory (export "memory") 17)
      (func (export "run")
        (local $len i32)
        (local.set $len (i32.load (i32.const 0)))
        (memory.copy (i32.const 0x100004) (i32.const 4) (local.get $len))
        (i32.store (i32.const 0x100000) (local.get $len))))
    "#;

    /// WAT module that loops forever — used to exercise CPU/time limits.
    const SPIN_WAT: &str = r#"
    (module
      (memory (export "memory") 17)
      (func (export "run") (loop $l (br $l))))
    "#;

    /// WAT module that imports a host function it should never get.
    const IMPORT_WAT: &str = r#"
    (module
      (import "host" "net" (func $net))
      (memory (export "memory") 17)
      (func (export "run") (call $net)))
    "#;

    #[test]
    fn echoes_json_round_trip() {
        let input = json!({"hello": "world", "n": 42});
        let out = run_wasm(ECHO_WAT.as_bytes(), &input, &SandboxLimits::default()).unwrap();
        assert_eq!(out, input);
    }

    #[test]
    fn infinite_loop_runs_out_of_fuel() {
        let limits = SandboxLimits {
            fuel: 1_000_000,
            ..SandboxLimits::default()
        };
        let err = run_wasm(SPIN_WAT.as_bytes(), &json!(null), &limits).unwrap_err();
        assert!(matches!(err, SandboxError::Trap(_)), "expected trap, got {err:?}");
    }

    #[test]
    fn infinite_loop_hits_epoch_deadline() {
        // Effectively unlimited fuel so the epoch watchdog is what stops it.
        let limits = SandboxLimits {
            fuel: u64::MAX,
            wall_ms: 50,
            ..SandboxLimits::default()
        };
        let err = run_wasm(SPIN_WAT.as_bytes(), &json!(null), &limits).unwrap_err();
        assert!(matches!(err, SandboxError::Trap(_)), "expected trap, got {err:?}");
    }

    #[test]
    fn memory_cap_blocks_oversized_module() {
        // Module needs 17 pages (>1 MiB); cap memory at 64 KiB so it can't grow.
        let limits = SandboxLimits {
            memory_bytes: 64 * 1024,
            ..SandboxLimits::default()
        };
        let err = run_wasm(ECHO_WAT.as_bytes(), &json!({"a": 1}), &limits).unwrap_err();
        assert!(
            matches!(err, SandboxError::Instantiate(_)),
            "expected instantiate failure, got {err:?}"
        );
    }

    #[test]
    fn module_with_imports_is_rejected() {
        let err = run_wasm(IMPORT_WAT.as_bytes(), &json!(null), &SandboxLimits::default())
            .unwrap_err();
        assert!(
            matches!(err, SandboxError::Instantiate(_)),
            "expected instantiate failure (no ambient capabilities), got {err:?}"
        );
    }
}
