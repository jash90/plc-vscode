//! End-to-end conformance for the `PRG_Test_ST.st` exerciser program.
//!
//! This program is a broad Structured Text smoke test (arithmetic, logic,
//! bit-string ops, selection, strings, `CASE`, `FOR`/`WHILE`, and the `TON`/`CTU`
//! standard function blocks) whose intent is to be *run* on CODESYS/TwinCAT and
//! observed online. Each `sLogNN` holds a human-readable result string. The
//! expected values below are the ground truth a real IEC 61131-3 compiler
//! produces — note the integral REALs render with a trailing `.0`
//! (`REAL_TO_STRING(1024.0)` -> `"1024.0"`), which is where the source file's own
//! `// oczekiwane` comments are inaccurate.

use plc_runtime::{Runtime, Value};

const SOURCE: &str = include_str!("fixtures/prg_test_st.st");

fn run(scans: u64) -> Runtime {
    let mut runtime = Runtime::from_source(SOURCE);
    runtime.set_scan_interval_ms(100);
    runtime.run_scans(scans);
    runtime
}

fn log(runtime: &Runtime, name: &str) -> String {
    match runtime.value(name) {
        Some(Value::Str(text)) => text.clone(),
        other => panic!("{name} expected a STRING, got {other:?}"),
    }
}

#[test]
fn computes_all_static_log_strings_like_a_real_st_compiler() {
    let runtime = run(25);

    let expected: &[(&str, &str)] = &[
        ("sLog01", "2 + 3 = 5"),
        ("sLog02", "2 * 3 = 6"),
        ("sLog03", "3 - 2 = 1"),
        ("sLog04", "7.0 / 2.0 = 3.5"),
        ("sLog05", "17 MOD 5 = 2"),
        // Integral REALs keep a trailing `.0`, matching CODESYS REAL_TO_STRING.
        ("sLog06", "2 ** 10 = 1024.0"),
        ("sLog07", "SQRT(144) = 12.0"),
        ("sLog08", "ABS(-3.5) = 3.5"),
        ("sLog09", "2 < 3 = TRUE"),
        ("sLog10", "TRUE AND FALSE = FALSE"),
        ("sLog11", "TRUE OR FALSE = TRUE"),
        ("sLog12", "TRUE XOR FALSE = TRUE"),
        ("sLog13", "NOT TRUE = FALSE"),
        ("sLog14", "15 AND 240 = 0"),
        ("sLog15", "15 OR 240 = 255"),
        ("sLog16", "15 SHL 4 = 240"),
        ("sLog17", "240 SHR 4 = 15"),
        ("sLog18", "MAX(5,3) = 5.0"),
        ("sLog19", "MIN(5,3) = 3.0"),
        ("sLog20", "LIMIT(0,15,10) = 10.0"),
        ("sLog21", "SEL(TRUE,1,2) = 2.0"),
        ("sLog22", "CONCAT = Jan Kowalski"),
        ("sLog23", "LEN(Jan Kowalski) = 12"),
        ("sLog24", "LEFT(...,3) = Jan"),
        ("sLog25", "suma 1..10 (FOR) = 55"),
        ("sLog26", "5! (FOR) = 120"),
        ("sLog27", "podwojen do >100 (WHILE) = 7"),
        ("sLog28", "CASE(2) = dwa"),
    ];

    for (name, value) in expected {
        assert_eq!(&log(&runtime, name), value, "{name} mismatch");
    }
}

#[test]
fn one_shot_loop_block_runs_exactly_once() {
    // The IF (xInit) block recomputes the loop results on the first scan only;
    // they must persist and not be recomputed afterwards.
    let after_one = run(1);
    assert_eq!(log(&after_one, "sLog25"), "suma 1..10 (FOR) = 55");
    assert_eq!(after_one.value("xInit"), Some(&Value::Bool(false)));

    let after_many = run(30);
    assert_eq!(log(&after_many, "sLog26"), "5! (FOR) = 120");
    assert_eq!(after_many.value("iSilnia"), Some(&Value::Int(120)));
}

#[test]
fn timer_and_counter_function_blocks_reach_steady_state() {
    // TON(PT=2s) at a 100 ms scan interval asserts Q after ~2 s of IN=TRUE;
    // CTU counts a rising edge every other scan (xTakt toggles each scan).
    let runtime = run(25);
    assert_eq!(log(&runtime, "sLog29"), "TON: ET = T#2s  Q = TRUE");
    assert_eq!(log(&runtime, "sLog30"), "CTU: CV = 13  Q = TRUE");
}

#[test]
fn timer_and_counter_are_still_ramping_on_the_first_scan() {
    let runtime = run(1);
    assert_eq!(log(&runtime, "sLog29"), "TON: ET = T#0ms  Q = FALSE");
    assert_eq!(log(&runtime, "sLog30"), "CTU: CV = 1  Q = FALSE");
}
