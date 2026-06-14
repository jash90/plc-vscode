use plc_runtime::Value;
use plc_runtime::stdlib::{call, is_standard_function};

#[test]
fn math_functions() {
    assert_eq!(call("ABS", &[Value::Int(-5)]), Value::Int(5));
    assert_eq!(call("abs", &[Value::Real(-2.5)]), Value::Real(2.5));
    assert_eq!(call("SQRT", &[Value::Real(9.0)]), Value::Real(3.0));
    assert_eq!(call("MIN", &[Value::Int(3), Value::Int(7)]), Value::Int(3));
    assert_eq!(call("MAX", &[Value::Int(3), Value::Int(7)]), Value::Int(7));
}

#[test]
fn selection_functions() {
    assert_eq!(
        call("LIMIT", &[Value::Int(0), Value::Int(15), Value::Int(10)]),
        Value::Int(10)
    );
    assert_eq!(
        call("SEL", &[Value::Bool(true), Value::Int(1), Value::Int(2)]),
        Value::Int(2)
    );
}

#[test]
fn string_functions() {
    assert_eq!(call("LEN", &[Value::Str("abc".to_owned())]), Value::Int(3));
    assert_eq!(
        call(
            "CONCAT",
            &[Value::Str("ab".to_owned()), Value::Str("cd".to_owned())]
        ),
        Value::Str("abcd".to_owned())
    );
}

#[test]
fn conversion_functions() {
    assert_eq!(call("INT_TO_REAL", &[Value::Int(3)]), Value::Real(3.0));
    assert_eq!(call("REAL_TO_INT", &[Value::Real(3.9)]), Value::Int(3));
    assert_eq!(call("BOOL_TO_INT", &[Value::Bool(true)]), Value::Int(1));
    assert_eq!(
        call("INT_TO_STRING", &[Value::Int(42)]),
        Value::Str("42".to_owned())
    );
}

#[test]
fn unknown_or_mismatched_calls_are_unknown() {
    assert_eq!(call("NOPE", &[Value::Int(1)]), Value::Unknown);
    assert_eq!(call("ABS", &[]), Value::Unknown);
    assert!(is_standard_function("limit"));
    assert!(!is_standard_function("nope"));
}
