//! Runtime state coverage derived from `PRG_Test_ST.st`: the post-scan values of
//! the working variables, scan/order dependence, determinism, and execution
//! through the `ExecutionEngine` port (a different entry point than
//! `prg_test_st.rs`, which reads the sLog watch via `Runtime` directly).

use plc_api::{ExecutionEngine, SourceDocument};
use plc_runtime::{Runtime, ScanRuntimeEngine, Value};

const FIXTURE: &str = include_str!("fixtures/prg_test_st.st");

fn run(scans: u64) -> Runtime {
    let mut runtime = Runtime::from_source(FIXTURE);
    runtime.set_scan_interval_ms(100);
    runtime.run_scans(scans);
    runtime
}

fn int(runtime: &Runtime, name: &str) -> i64 {
    match runtime.value(name) {
        Some(Value::Int(value)) => *value,
        other => panic!("{name} expected Int, got {other:?}"),
    }
}

fn string(runtime: &Runtime, name: &str) -> String {
    match runtime.value(name) {
        Some(Value::Str(value)) => value.clone(),
        other => panic!("{name} expected Str, got {other:?}"),
    }
}

#[test]
fn one_shot_loop_results_are_stable() {
    let runtime = run(25);
    assert_eq!(int(&runtime, "iSuma"), 55); // FOR sum 1..10
    assert_eq!(int(&runtime, "iSilnia"), 120); // 5!
    assert_eq!(int(&runtime, "diWynik"), 7); // doublings of 1 until > 100
}

#[test]
fn last_write_wins_for_reused_working_vars() {
    let runtime = run(25);
    // wWynik: AND(0) -> OR(255) -> SHL(240) -> SHR(15); last write is SHR.
    assert_eq!(int(&runtime, "wWynik"), 15);
    // rWynik: ... -> SEL(TRUE,1.0,2.0) is the final assignment.
    assert_eq!(runtime.value("rWynik"), Some(&Value::Real(2.0)));
    // iDlugosc := LEN('Jan Kowalski').
    assert_eq!(int(&runtime, "iDlugosc"), 12);
    // sTekst: 'Jan Kowalski' -> LEFT 'Jan' -> CASE overwrites to 'dwa'.
    assert_eq!(string(&runtime, "sTekst"), "dwa");
}

#[test]
fn one_shot_flag_is_cleared_after_first_scan() {
    assert_eq!(run(1).value("xInit"), Some(&Value::Bool(false)));
    assert_eq!(run(25).value("xInit"), Some(&Value::Bool(false)));
}

#[test]
fn iwynik_is_scan_order_dependent() {
    // Scan 1: the IF(xInit) WHILE runs last and leaves iWynik at 128.
    assert_eq!(int(&run(1), "iWynik"), 128);
    // Later scans: IF skipped, so the last write is `iWynik := 2;` before CASE.
    assert_eq!(int(&run(25), "iWynik"), 2);
}

#[test]
fn variable_lookup_is_case_insensitive() {
    let runtime = run(25);
    assert_eq!(runtime.value("isuma"), runtime.value("iSuma"));
    assert_eq!(runtime.value("DIWYNIK"), runtime.value("diWynik"));
}

#[test]
fn ctu_count_value_progresses_with_scans() {
    // CV rises one per rising edge of xTakt (every other scan), reported in sLog30.
    assert_eq!(string(&run(1), "sLog30"), "CTU: CV = 1  Q = FALSE");
    assert_eq!(string(&run(5), "sLog30"), "CTU: CV = 3  Q = FALSE");
    assert_eq!(string(&run(25), "sLog30"), "CTU: CV = 13  Q = TRUE");
}

#[test]
fn timer_value_reaches_steady_state() {
    assert_eq!(string(&run(1), "sLog29"), "TON: ET = T#0ms  Q = FALSE");
    assert_eq!(string(&run(25), "sLog29"), "TON: ET = T#2s  Q = TRUE");
}

#[test]
fn execution_is_deterministic_across_runs() {
    let first = run(25).watch();
    let second = run(25).watch();
    assert_eq!(first, second);
    assert!(!first.is_empty());
}

#[test]
fn runs_through_execution_engine_port_with_correct_watch() {
    let document = SourceDocument::new("file:///prg.st", 0, FIXTURE);
    let mut engine = ScanRuntimeEngine::default();
    engine.load(&document).expect("loads");
    engine.set_scan_interval_ms(100);
    engine.run_scans(25);

    let watch = engine.watch();
    let has = |line: &str| watch.iter().any(|l| l == line);
    assert!(has("sLog01 = 2 + 3 = 5"), "watch: {watch:?}");
    assert!(has("sLog06 = 2 ** 10 = 1024.0"));
    assert!(has("sLog25 = suma 1..10 (FOR) = 55"));
    assert!(has("sLog28 = CASE(2) = dwa"));
    assert!(has("sLog29 = TON: ET = T#2s  Q = TRUE"));
}

#[test]
fn engine_watch_is_empty_before_load() {
    let engine = ScanRuntimeEngine::default();
    assert!(engine.watch().is_empty());
}
