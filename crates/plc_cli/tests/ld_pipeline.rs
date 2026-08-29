//! PLC-109 — End-to-end LD pipeline: converted ST actually executes FBs and
//! wires their outputs to user variables (differential evidence that lowering
//! is complete).

use plc_api::SourceDocument;
use plc_lang::LanguageRegistry;
use plc_runtime::Runtime;

/// One rung: Start ──TON(PT=T#200ms)──(Q→Done).
const TIMER_LD: &str = r#"{
    "name": "TimerTest",
    "rungs": [
        {
            "branches": [{ "elements": [{ "name": "Start", "negated": false }] }],
            "outputs": [
                {
                    "kind": "block",
                    "fb_type": "TON",
                    "instance": "Delay",
                    "inputs": [
                        { "name": "IN", "value": "Start" },
                        { "name": "PT", "value": "T#200ms" }
                    ],
                    "outputs": [{ "name": "Q", "value": "Done" }]
                }
            ]
        }
    ]
}"#;

#[test]
fn timer_output_reaches_variable_through_runtime() {
    let registry = LanguageRegistry::with_builtins();
    let result = registry.convert(
        "ld",
        "st",
        &SourceDocument::new("file:///timer.ld".to_owned(), 0, TIMER_LD.to_owned()),
    );
    assert!(
        result.error.is_none(),
        "conversion failed: {:?} {:?}",
        result.error,
        result.diagnostics
    );

    // The rendered ST must declare the instance with its FB type (not INT).
    assert!(
        result.text.contains("Delay : TON;"),
        "ST should declare the FB instance: {}",
        result.text
    );

    let mut runtime = Runtime::from_source(&result.text);
    runtime.set_scan_interval_ms(100);
    runtime.set_input("Start", plc_runtime::Value::Bool(true));

    // Scan semantics: the clock ticks before logic, so scan n runs at
    // t = n·100ms; TON fires at et >= pt. Exact sequence: F, F, T.
    runtime.run_scans(1);
    assert!(
        !runtime.watch().iter().any(|l| l.contains("Done = TRUE")),
        "scan 1 (t=0): Done must be FALSE: {:?}",
        runtime.watch()
    );
    runtime.run_scans(1);
    assert!(
        !runtime.watch().iter().any(|l| l.contains("Done = TRUE")),
        "scan 2 (t=100ms < PT): Done must still be FALSE: {:?}",
        runtime.watch()
    );
    runtime.run_scans(1);
    assert!(
        runtime.watch().iter().any(|l| l.contains("Done = TRUE")),
        "scan 3 (t=200ms >= PT): Done must be TRUE: {:?}",
        runtime.watch()
    );
}
