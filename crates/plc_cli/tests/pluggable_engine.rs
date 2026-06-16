//! Proves the CLI driver runs with a third-party execution engine: any
//! `plc_api::ExecutionEngine` can drive `run_with`, independent of plc_runtime.

use plc_api::{Diagnostic, ExecutionEngine, SourceDocument};
use plc_cli::run_with;
use plc_compiler_core::CompilerCore;

/// A fake engine standing in for a third-party compiler/runtime (e.g. an LLVM
/// JIT or a remote PLC). It records what the driver asked it to do.
#[derive(Default)]
struct FakeEngine {
    loaded: bool,
    interval_ms: i64,
    scans: u64,
    snapshot: Vec<String>,
}

impl ExecutionEngine for FakeEngine {
    fn load(&mut self, _document: &SourceDocument) -> Result<(), Vec<Diagnostic>> {
        self.loaded = true;
        self.snapshot = vec!["x = 1".to_owned()];
        Ok(())
    }
    fn set_scan_interval_ms(&mut self, scan_interval_ms: i64) {
        self.interval_ms = scan_interval_ms;
    }
    fn run_scans(&mut self, cycles: u64) {
        self.scans = cycles;
    }
    fn set_input(&mut self, _name: &str, _value: &str) {}
    fn watch(&self) -> Vec<String> {
        self.snapshot.clone()
    }
}

#[test]
fn cli_driver_runs_with_a_third_party_engine() {
    // A clean program so the CompilerCore diagnostics gate passes and the driver
    // proceeds to the engine.
    let document = SourceDocument::new(
        "file:///x.st",
        0,
        "PROGRAM Main\nVAR\n    x : INT;\nEND_VAR\nx := 1;\nEND_PROGRAM\n",
    );
    let service = CompilerCore;
    let mut engine = FakeEngine::default();

    run_with(&service, &mut engine, &document, 5).expect("clean program runs");

    assert!(engine.loaded, "engine.load was driven");
    assert_eq!(engine.interval_ms, plc_cli::SCAN_INTERVAL_MS);
    assert_eq!(engine.scans, 5);
    assert_eq!(engine.watch(), vec!["x = 1".to_owned()]);
}

#[test]
fn diagnostics_gate_blocks_a_broken_program_before_load() {
    // Missing END_PROGRAM -> the analyze gate fails and the engine is never loaded.
    let document = SourceDocument::new(
        "file:///bad.st",
        0,
        "PROGRAM Bad\nVAR\n    x : INT;\nEND_VAR",
    );
    let service = CompilerCore;
    let mut engine = FakeEngine::default();

    assert!(run_with(&service, &mut engine, &document, 1).is_err());
    assert!(!engine.loaded, "engine must not load when diagnostics fail");
}
