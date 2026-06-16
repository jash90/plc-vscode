//! Focused coverage for interpreter constructs beyond the broad fixture:
//! operator precedence, ELSIF chains, REPEAT/UNTIL, descending FOR…BY, and
//! EXIT/CONTINUE inside loops.

use plc_runtime::{Runtime, Value};

fn run_once(body: &str) -> Runtime {
    let source = format!(
        "PROGRAM Main\nVAR\n    a : INT;\n    b : INT;\n    c : INT;\n    r : INT;\n    f : BOOL;\nEND_VAR\n{body}\nEND_PROGRAM\n"
    );
    let mut runtime = Runtime::from_source(&source);
    runtime.run_scan();
    runtime
}

#[test]
fn respects_operator_precedence() {
    let runtime = run_once("r := 2 + 3 * 4;");
    assert_eq!(runtime.value("r"), Some(&Value::Int(14)));

    // Comparison binds looser than arithmetic; AND looser than comparison.
    let runtime = run_once("f := 2 + 1 > 2 AND 5 - 1 = 4;");
    assert_eq!(runtime.value("f"), Some(&Value::Bool(true)));

    // Unary NOT binds tighter than AND.
    let runtime = run_once("f := NOT FALSE AND TRUE;");
    assert_eq!(runtime.value("f"), Some(&Value::Bool(true)));

    // Parentheses override precedence.
    let runtime = run_once("r := (2 + 3) * 4;");
    assert_eq!(runtime.value("r"), Some(&Value::Int(20)));
}

#[test]
fn elsif_chain_selects_first_true_branch() {
    let body = "a := 2;\nIF a = 1 THEN r := 10;\nELSIF a = 2 THEN r := 20;\nELSIF a = 3 THEN r := 30;\nELSE r := 99;\nEND_IF";
    assert_eq!(run_once(body).value("r"), Some(&Value::Int(20)));

    let body = "a := 7;\nIF a = 1 THEN r := 10;\nELSIF a = 2 THEN r := 20;\nELSE r := 99;\nEND_IF";
    assert_eq!(run_once(body).value("r"), Some(&Value::Int(99)));
}

#[test]
fn repeat_until_runs_at_least_once() {
    let body =
        "r := 0;\na := 0;\nREPEAT\n    r := r + a;\n    a := a + 1;\nUNTIL a > 4\nEND_REPEAT";
    // 0+1+2+3+4 = 10
    assert_eq!(run_once(body).value("r"), Some(&Value::Int(10)));
}

#[test]
fn descending_for_with_step() {
    let body = "r := 0;\nFOR a := 10 TO 0 BY -2 DO\n    r := r + a;\nEND_FOR";
    // 10+8+6+4+2+0 = 30
    assert_eq!(run_once(body).value("r"), Some(&Value::Int(30)));
}

#[test]
fn exit_and_continue_in_loops() {
    // EXIT breaks the FOR loop early at a == 5.
    let body =
        "r := 0;\nFOR a := 1 TO 100 DO\n    IF a = 5 THEN EXIT; END_IF\n    r := r + 1;\nEND_FOR";
    assert_eq!(run_once(body).value("r"), Some(&Value::Int(4)));

    // CONTINUE skips even increments, counting only odd iterations 1..10.
    let body = "r := 0;\nFOR a := 1 TO 10 DO\n    IF a MOD 2 = 0 THEN CONTINUE; END_IF\n    r := r + 1;\nEND_FOR";
    assert_eq!(run_once(body).value("r"), Some(&Value::Int(5)));
}

#[test]
fn nested_case_with_ranges() {
    let body =
        "a := 7;\nCASE a OF\n    1, 2, 3: r := 1;\n    4..9: r := 2;\nELSE\n    r := 3;\nEND_CASE";
    assert_eq!(run_once(body).value("r"), Some(&Value::Int(2)));
}
