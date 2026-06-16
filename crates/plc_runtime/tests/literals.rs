//! Literal and type-default parsing for radix, typed, and duration literals.

use plc_runtime::Value;

#[test]
fn parses_radix_and_typed_integer_literals() {
    assert_eq!(Value::parse_literal("16#000F"), Some(Value::Int(15)));
    assert_eq!(Value::parse_literal("16#00F0"), Some(Value::Int(240)));
    assert_eq!(Value::parse_literal("2#1010_0110"), Some(Value::Int(166)));
    assert_eq!(Value::parse_literal("8#17"), Some(Value::Int(15)));
    assert_eq!(Value::parse_literal("WORD#16#0F"), Some(Value::Int(15)));
    assert_eq!(Value::parse_literal("INT#-5"), Some(Value::Int(-5)));
    assert_eq!(Value::parse_literal("BOOL#1"), Some(Value::Bool(true)));
    assert_eq!(Value::parse_literal("LREAL#3.5"), Some(Value::Real(3.5)));
}

#[test]
fn parses_decimal_literals_with_separators() {
    assert_eq!(Value::parse_literal("1_000"), Some(Value::Int(1000)));
    assert_eq!(Value::parse_literal("7.0"), Some(Value::Real(7.0)));
    assert_eq!(Value::parse_literal("-3.5"), Some(Value::Real(-3.5)));
}

#[test]
fn parses_compound_duration_literals() {
    assert_eq!(Value::parse_literal("T#2s"), Some(Value::Time(2_000)));
    assert_eq!(Value::parse_literal("T#45m"), Some(Value::Time(2_700_000)));
    assert_eq!(
        Value::parse_literal("T#1h30m"),
        Some(Value::Time(5_400_000))
    );
    assert_eq!(Value::parse_literal("T#100ms"), Some(Value::Time(100)));
    assert_eq!(Value::parse_literal("T#1d"), Some(Value::Time(86_400_000)));
    assert_eq!(
        Value::parse_literal("TIME#2s500ms"),
        Some(Value::Time(2_500))
    );
}

#[test]
fn bit_string_types_default_to_integer_zero() {
    assert_eq!(Value::type_default("WORD"), Value::Int(0));
    assert_eq!(Value::type_default("BYTE"), Value::Int(0));
    assert_eq!(Value::type_default("DWORD"), Value::Int(0));
}
