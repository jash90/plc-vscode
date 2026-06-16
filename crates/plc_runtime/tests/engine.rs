//! The default `ScanRuntimeEngine` drives the program through the `plc_api`
//! `ExecutionEngine` port (the seam a third party would target).

use plc_api::{ExecutionEngine, SourceDocument};
use plc_runtime::ScanRuntimeEngine;

#[test]
fn scan_runtime_engine_executes_through_the_port() {
    let document = SourceDocument::new(
        "file:///counter.st",
        0,
        "PROGRAM Main\nVAR\n    Count : INT;\nEND_VAR\nCount := Count + 1;\nEND_PROGRAM\n",
    );

    let mut engine: Box<dyn ExecutionEngine> = Box::new(ScanRuntimeEngine::default());
    engine.load(&document).expect("loads");
    engine.set_scan_interval_ms(10);
    engine.run_scans(4);

    let watch = engine.watch();
    assert!(
        watch.iter().any(|line| line == "Count = 4"),
        "expected `Count = 4` in watch, got {watch:?}"
    );
}
