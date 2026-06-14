//! IEC 61131-3 standard function library (MVP).
//!
//! These are **pure** functions over [`Value`]: conversion, math, selection,
//! and string helpers. Stateful function blocks (timers, counters, edge
//! detectors) live elsewhere. Functions are dispatched by name via [`call`] so
//! the runtime and semantic layer can share one definition of the standard set.

use crate::Value;

/// The standard functions recognized by this MVP, for type-check participation.
pub const STANDARD_FUNCTIONS: &[&str] = &[
    "ABS",
    "SQRT",
    "MIN",
    "MAX",
    "LIMIT",
    "SEL",
    "LEN",
    "CONCAT",
    "INT_TO_REAL",
    "REAL_TO_INT",
    "BOOL_TO_INT",
    "INT_TO_STRING",
];

/// Whether `name` is a recognized standard function (case-insensitive).
pub fn is_standard_function(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    STANDARD_FUNCTIONS
        .iter()
        .any(|candidate| *candidate == upper)
}

/// Evaluate a standard function by name. Unknown names or arity/type mismatches
/// yield [`Value::Unknown`].
pub fn call(name: &str, args: &[Value]) -> Value {
    match name.to_ascii_uppercase().as_str() {
        "ABS" => unary(args, abs),
        "SQRT" => unary(args, sqrt),
        "MIN" => binary(args, min),
        "MAX" => binary(args, max),
        "LIMIT" => limit(args),
        "SEL" => sel(args),
        "LEN" => unary(args, len),
        "CONCAT" => binary(args, concat),
        "INT_TO_REAL" => unary(args, int_to_real),
        "REAL_TO_INT" => unary(args, real_to_int),
        "BOOL_TO_INT" => unary(args, bool_to_int),
        "INT_TO_STRING" => unary(args, int_to_string),
        _ => Value::Unknown,
    }
}

fn unary(args: &[Value], f: fn(&Value) -> Value) -> Value {
    match args {
        [value] => f(value),
        _ => Value::Unknown,
    }
}

fn binary(args: &[Value], f: fn(&Value, &Value) -> Value) -> Value {
    match args {
        [left, right] => f(left, right),
        _ => Value::Unknown,
    }
}

fn abs(value: &Value) -> Value {
    match value {
        Value::Int(v) => Value::Int(v.abs()),
        Value::Real(v) => Value::Real(v.abs()),
        _ => Value::Unknown,
    }
}

fn sqrt(value: &Value) -> Value {
    match value {
        Value::Int(v) if *v >= 0 => Value::Real((*v as f64).sqrt()),
        Value::Real(v) if *v >= 0.0 => Value::Real(v.sqrt()),
        _ => Value::Unknown,
    }
}

fn min(left: &Value, right: &Value) -> Value {
    match (left, right) {
        (Value::Int(a), Value::Int(b)) => Value::Int(*a.min(b)),
        (Value::Real(a), Value::Real(b)) => Value::Real(a.min(*b)),
        _ => Value::Unknown,
    }
}

fn max(left: &Value, right: &Value) -> Value {
    match (left, right) {
        (Value::Int(a), Value::Int(b)) => Value::Int(*a.max(b)),
        (Value::Real(a), Value::Real(b)) => Value::Real(a.max(*b)),
        _ => Value::Unknown,
    }
}

fn limit(args: &[Value]) -> Value {
    match args {
        [mn, input, mx] => min(&max(input, mn), mx),
        _ => Value::Unknown,
    }
}

fn sel(args: &[Value]) -> Value {
    match args {
        [Value::Bool(g), in0, in1] => {
            if *g {
                in1.clone()
            } else {
                in0.clone()
            }
        }
        _ => Value::Unknown,
    }
}

fn len(value: &Value) -> Value {
    match value {
        Value::Str(s) => Value::Int(s.chars().count() as i64),
        _ => Value::Unknown,
    }
}

fn concat(left: &Value, right: &Value) -> Value {
    match (left, right) {
        (Value::Str(a), Value::Str(b)) => Value::Str(format!("{a}{b}")),
        _ => Value::Unknown,
    }
}

fn int_to_real(value: &Value) -> Value {
    match value {
        Value::Int(v) => Value::Real(*v as f64),
        _ => Value::Unknown,
    }
}

fn real_to_int(value: &Value) -> Value {
    match value {
        Value::Real(v) => Value::Int(*v as i64),
        _ => Value::Unknown,
    }
}

fn bool_to_int(value: &Value) -> Value {
    match value {
        Value::Bool(v) => Value::Int(if *v { 1 } else { 0 }),
        _ => Value::Unknown,
    }
}

fn int_to_string(value: &Value) -> Value {
    match value {
        Value::Int(v) => Value::Str(v.to_string()),
        _ => Value::Unknown,
    }
}
