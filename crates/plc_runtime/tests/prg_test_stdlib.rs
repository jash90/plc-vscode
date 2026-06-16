//! Standard-function coverage for the exact argument shapes/values that appear
//! in `PRG_Test_ST.st` (complementing the generic cases in `stdlib.rs`).

use plc_runtime::Value;
use plc_runtime::stdlib::{call, is_standard_function};

#[test]
fn nested_concat_builds_full_name() {
    // CONCAT(sImie, CONCAT(' ', sNazw))
    let inner = call(
        "CONCAT",
        &[
            Value::Str(" ".to_owned()),
            Value::Str("Kowalski".to_owned()),
        ],
    );
    let full = call("CONCAT", &[Value::Str("Jan".to_owned()), inner]);
    assert_eq!(full, Value::Str("Jan Kowalski".to_owned()));
}

#[test]
fn len_of_full_name_is_twelve() {
    assert_eq!(
        call("LEN", &[Value::Str("Jan Kowalski".to_owned())]),
        Value::Int(12)
    );
}

#[test]
fn left_of_full_name_is_jan() {
    assert_eq!(
        call(
            "LEFT",
            &[Value::Str("Jan Kowalski".to_owned()), Value::Int(3)]
        ),
        Value::Str("Jan".to_owned())
    );
}

#[test]
fn word_to_string_of_bitwise_results() {
    assert_eq!(
        call("WORD_TO_STRING", &[Value::Int(0)]),
        Value::Str("0".to_owned())
    );
    assert_eq!(
        call("WORD_TO_STRING", &[Value::Int(255)]),
        Value::Str("255".to_owned())
    );
}

#[test]
fn dint_to_string_of_loop_results() {
    assert_eq!(
        call("DINT_TO_STRING", &[Value::Int(120)]),
        Value::Str("120".to_owned())
    );
    assert_eq!(
        call("DINT_TO_STRING", &[Value::Int(7)]),
        Value::Str("7".to_owned())
    );
}

#[test]
fn int_to_string_of_arithmetic_results() {
    for (n, s) in [(5, "5"), (6, "6"), (1, "1"), (2, "2")] {
        assert_eq!(
            call("INT_TO_STRING", &[Value::Int(n)]),
            Value::Str(s.to_owned())
        );
    }
}

#[test]
fn bool_to_string_false_branch() {
    assert_eq!(
        call("BOOL_TO_STRING", &[Value::Bool(false)]),
        Value::Str("FALSE".to_owned())
    );
}

#[test]
fn selection_functions_on_reals_like_the_file() {
    assert_eq!(
        call("MAX", &[Value::Real(5.0), Value::Real(3.0)]),
        Value::Real(5.0)
    );
    assert_eq!(
        call("MIN", &[Value::Real(5.0), Value::Real(3.0)]),
        Value::Real(3.0)
    );
    assert_eq!(
        call(
            "LIMIT",
            &[Value::Real(0.0), Value::Real(15.0), Value::Real(10.0)]
        ),
        Value::Real(10.0)
    );
    assert_eq!(
        call(
            "SEL",
            &[Value::Bool(true), Value::Real(1.0), Value::Real(2.0)]
        ),
        Value::Real(2.0)
    );
}

#[test]
fn abs_of_negative_real_from_the_file() {
    assert_eq!(call("ABS", &[Value::Real(-3.5)]), Value::Real(3.5));
}

#[test]
fn to_string_of_counter_value() {
    assert_eq!(
        call("TO_STRING", &[Value::Int(13)]),
        Value::Str("13".to_owned())
    );
}

#[test]
fn mod_is_an_operator_not_a_standard_function() {
    // The file uses `17 MOD 5` as an operator; MOD is not a stdlib function.
    assert!(!is_standard_function("MOD"));
    assert_eq!(
        call("MOD", &[Value::Int(17), Value::Int(5)]),
        Value::Unknown
    );
}
