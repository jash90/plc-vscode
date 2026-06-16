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
fn to_string_family_matches_codesys_formatting() {
    // REAL_TO_STRING keeps the decimal point + trailing zero for whole numbers.
    assert_eq!(
        call("REAL_TO_STRING", &[Value::Real(1024.0)]),
        Value::Str("1024.0".to_owned())
    );
    assert_eq!(
        call("REAL_TO_STRING", &[Value::Real(12.0)]),
        Value::Str("12.0".to_owned())
    );
    assert_eq!(
        call("REAL_TO_STRING", &[Value::Real(3.5)]),
        Value::Str("3.5".to_owned())
    );
    assert_eq!(
        call("BOOL_TO_STRING", &[Value::Bool(true)]),
        Value::Str("TRUE".to_owned())
    );
    assert_eq!(
        call("WORD_TO_STRING", &[Value::Int(240)]),
        Value::Str("240".to_owned())
    );
    assert_eq!(
        call("DINT_TO_STRING", &[Value::Int(55)]),
        Value::Str("55".to_owned())
    );
    assert_eq!(
        call("TIME_TO_STRING", &[Value::Time(2000)]),
        Value::Str("T#2s".to_owned())
    );
    assert_eq!(
        call("TIME_TO_STRING", &[Value::Time(0)]),
        Value::Str("T#0ms".to_owned())
    );
    assert_eq!(
        call("TO_STRING", &[Value::Int(10)]),
        Value::Str("10".to_owned())
    );
}

#[test]
fn numeric_and_bit_and_string_functions() {
    assert_eq!(
        call("EXPT", &[Value::Real(2.0), Value::Real(10.0)]),
        Value::Real(1024.0)
    );
    assert_eq!(
        call("SHL", &[Value::Int(15), Value::Int(4)]),
        Value::Int(240)
    );
    assert_eq!(
        call("SHR", &[Value::Int(240), Value::Int(4)]),
        Value::Int(15)
    );
    assert_eq!(
        call(
            "LEFT",
            &[Value::Str("Jan Kowalski".to_owned()), Value::Int(3)]
        ),
        Value::Str("Jan".to_owned())
    );
    assert_eq!(
        call(
            "CONCAT",
            &[
                Value::Str("a".to_owned()),
                Value::Str("b".to_owned()),
                Value::Str("c".to_owned())
            ]
        ),
        Value::Str("abc".to_owned())
    );
}

#[test]
fn unknown_or_mismatched_calls_are_unknown() {
    assert_eq!(call("NOPE", &[Value::Int(1)]), Value::Unknown);
    assert_eq!(call("ABS", &[]), Value::Unknown);
    assert!(is_standard_function("limit"));
    assert!(!is_standard_function("nope"));
}
