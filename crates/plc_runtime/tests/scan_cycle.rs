use plc_runtime::{Runtime, Value};

#[test]
fn retains_state_across_scan_cycles() {
    let mut runtime = Runtime::from_source(
        "PROGRAM Main\nVAR\n    Count : INT;\nEND_VAR\nCount := Count + 1;\nEND_PROGRAM\n",
    );

    runtime.run_scans(3);

    assert_eq!(runtime.value("Count"), Some(&Value::Int(3)));
    assert_eq!(runtime.scan_count(), 3);
}

#[test]
fn input_scan_latches_staged_inputs_before_logic() {
    let mut runtime = Runtime::from_source(
        "PROGRAM Main\nVAR_INPUT\n    Start : BOOL;\nEND_VAR\nVAR\n    Running : BOOL;\nEND_VAR\nRunning := Start;\nEND_PROGRAM\n",
    );

    runtime.set_input("Start", Value::Bool(true));
    runtime.run_scan();

    assert_eq!(runtime.value("Running"), Some(&Value::Bool(true)));
}

#[test]
fn output_scan_reports_declared_outputs() {
    let mut runtime = Runtime::from_source(
        "PROGRAM Main\nVAR_OUTPUT\n    Lamp : BOOL;\nEND_VAR\nLamp := TRUE;\nEND_PROGRAM\n",
    );

    let snapshot = runtime.run_scan();

    assert!(snapshot.iter().any(|line| line == "Lamp = TRUE"));
}
