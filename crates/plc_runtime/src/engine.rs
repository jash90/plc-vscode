//! [`ExecutionEngine`] adapter over the deterministic scan-cycle [`Runtime`].
//!
//! This is the reference implementation of the `plc_api` execution port: the
//! CLI (or any host) drives a `dyn ExecutionEngine` and gets the interpreter
//! runtime by default, but can swap in a different compiler/runtime (bytecode
//! VM, LLVM JIT, remote PLC) without changing the driver.

use plc_api::{Diagnostic, ExecutionEngine, SourceDocument};

use crate::{Runtime, Value};

/// Default [`ExecutionEngine`]: builds a [`Runtime`] from source and runs scan
/// cycles. The scan interval may be configured before or after `load`.
#[derive(Debug, Default)]
pub struct ScanRuntimeEngine {
    runtime: Option<Runtime>,
    scan_interval_ms: Option<i64>,
}

impl ExecutionEngine for ScanRuntimeEngine {
    fn load(&mut self, document: &SourceDocument) -> Result<(), Vec<Diagnostic>> {
        let mut runtime = Runtime::from_source(document.text());
        if let Some(scan_interval_ms) = self.scan_interval_ms {
            runtime.set_scan_interval_ms(scan_interval_ms);
        }
        self.runtime = Some(runtime);
        Ok(())
    }

    fn set_scan_interval_ms(&mut self, scan_interval_ms: i64) {
        self.scan_interval_ms = Some(scan_interval_ms);
        if let Some(runtime) = self.runtime.as_mut() {
            runtime.set_scan_interval_ms(scan_interval_ms);
        }
    }

    fn run_scans(&mut self, cycles: u64) {
        if let Some(runtime) = self.runtime.as_mut() {
            runtime.run_scans(cycles);
        }
    }

    fn set_input(&mut self, name: &str, value: &str) {
        if let Some(runtime) = self.runtime.as_mut()
            && let Some(parsed) = Value::parse_literal(value)
        {
            runtime.set_input(name, parsed);
        }
    }

    fn watch(&self) -> Vec<String> {
        self.runtime
            .as_ref()
            .map(Runtime::watch)
            .unwrap_or_default()
    }
}
