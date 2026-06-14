use plc_runtime::{Runtime, Value};

const COUNTER: &str =
    "PROGRAM Main\nVAR\n    Count : INT;\nEND_VAR\nCount := Count + 1;\nEND_PROGRAM\n";

#[test]
fn forced_variable_overrides_logic_writes() {
    let mut runtime = Runtime::from_source(COUNTER);
    runtime.force("Count", Value::Int(100));

    runtime.run_scans(5);

    assert!(runtime.is_forced("Count"));
    assert_eq!(runtime.value("Count"), Some(&Value::Int(100)));
}

#[test]
fn releasing_a_force_resumes_logic() {
    let mut runtime = Runtime::from_source(COUNTER);
    runtime.force("Count", Value::Int(100));
    runtime.run_scan();
    runtime.unforce("Count");
    assert!(!runtime.is_forced("Count"));

    runtime.run_scan();
    // Logic resumes from the forced value: 100 + 1.
    assert_eq!(runtime.value("Count"), Some(&Value::Int(101)));
}

#[test]
fn inspect_reports_values_and_forced_flags() {
    let mut runtime = Runtime::from_source(COUNTER);
    runtime.force("Count", Value::Int(7));

    let snapshot = runtime.inspect();
    let count = snapshot
        .iter()
        .find(|entry| entry.name == "count")
        .expect("count variable present");
    assert_eq!(count.value, Value::Int(7));
    assert!(count.forced);
}
