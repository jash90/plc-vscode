//! Literal-parsing and type-default coverage for the elementary types and
//! initializers used in `PRG_Test_ST.st` (excluding hex/duration/WORD-default
//! already covered by `literals.rs`).

use plc_runtime::Value;

#[test]
fn parses_string_name_initializers() {
    assert_eq!(
        Value::parse_literal("'Jan'"),
        Some(Value::Str("Jan".to_owned()))
    );
    assert_eq!(
        Value::parse_literal("'Kowalski'"),
        Some(Value::Str("Kowalski".to_owned()))
    );
}

#[test]
fn parses_bool_initializers() {
    assert_eq!(Value::parse_literal("TRUE"), Some(Value::Bool(true)));
    assert_eq!(Value::parse_literal("FALSE"), Some(Value::Bool(false)));
}

#[test]
fn parses_int_initializers() {
    assert_eq!(Value::parse_literal("2"), Some(Value::Int(2)));
    assert_eq!(Value::parse_literal("3"), Some(Value::Int(3)));
}

#[test]
fn parses_real_initializers() {
    assert_eq!(Value::parse_literal("7.0"), Some(Value::Real(7.0)));
    assert_eq!(Value::parse_literal("2.0"), Some(Value::Real(2.0)));
}

#[test]
fn type_defaults_for_scalar_types() {
    assert_eq!(Value::type_default("INT"), Value::Int(0));
    assert_eq!(Value::type_default("DINT"), Value::Int(0));
    assert_eq!(Value::type_default("REAL"), Value::Real(0.0));
    assert_eq!(Value::type_default("BOOL"), Value::Bool(false));
    assert_eq!(Value::type_default("STRING"), Value::Str(String::new()));
    assert_eq!(Value::type_default("TIME"), Value::Time(0));
}

#[test]
fn type_default_for_function_block_type_is_unknown() {
    assert_eq!(Value::type_default("TON"), Value::Unknown);
    assert_eq!(Value::type_default("CTU"), Value::Unknown);
}
