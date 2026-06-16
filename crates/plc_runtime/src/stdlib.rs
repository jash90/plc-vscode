//! IEC 61131-3 standard function library (MVP).
//!
//! These are **pure** functions over [`Value`]: conversion, math, selection,
//! and string helpers. Stateful function blocks (timers, counters, edge
//! detectors) live elsewhere. Functions are dispatched by name via [`call`] so
//! the runtime and semantic layer can share one definition of the standard set.

use crate::Value;

/// The standard functions recognized by this MVP, for type-check participation.
///
/// Keep in sync with `plc_compiler_core`'s `STANDARD_FUNCTION_NAMES` (completion
/// / semantic-token classification) and `standard_signature` (signature help).
pub const STANDARD_FUNCTIONS: &[&str] = &[
    // math / numeric
    "ABS",
    "SQRT",
    "EXPT",
    "MIN",
    "MAX",
    "LIMIT",
    "SEL",
    // bit-string shifts
    "SHL",
    "SHR",
    // string
    "LEN",
    "CONCAT",
    "LEFT",
    "RIGHT",
    "MID",
    // numeric conversions
    "INT_TO_REAL",
    "REAL_TO_INT",
    "BOOL_TO_INT",
    // *_TO_STRING family
    "INT_TO_STRING",
    "DINT_TO_STRING",
    "REAL_TO_STRING",
    "LREAL_TO_STRING",
    "BOOL_TO_STRING",
    "WORD_TO_STRING",
    "BYTE_TO_STRING",
    "DWORD_TO_STRING",
    "TIME_TO_STRING",
    "TO_STRING",
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
        "EXPT" => binary(args, expt),
        "MIN" => binary(args, min),
        "MAX" => binary(args, max),
        "LIMIT" => limit(args),
        "SEL" => sel(args),
        "SHL" => binary(args, shl),
        "SHR" => binary(args, shr),
        "LEN" => unary(args, len),
        "CONCAT" => concat(args),
        "LEFT" => binary(args, left),
        "RIGHT" => binary(args, right),
        "MID" => mid(args),
        "INT_TO_REAL" => unary(args, int_to_real),
        "REAL_TO_INT" => unary(args, real_to_int),
        "BOOL_TO_INT" => unary(args, bool_to_int),
        // Integer/bit-string -> STRING (decimal rendering).
        "INT_TO_STRING" | "DINT_TO_STRING" | "SINT_TO_STRING" | "LINT_TO_STRING"
        | "UINT_TO_STRING" | "UDINT_TO_STRING" | "USINT_TO_STRING" | "ULINT_TO_STRING"
        | "WORD_TO_STRING" | "BYTE_TO_STRING" | "DWORD_TO_STRING" | "LWORD_TO_STRING" => {
            unary(args, int_to_string)
        }
        "REAL_TO_STRING" | "LREAL_TO_STRING" => unary(args, real_to_string_value),
        "BOOL_TO_STRING" => unary(args, bool_to_string),
        "TIME_TO_STRING" | "LTIME_TO_STRING" => unary(args, time_to_string_value),
        // Generic overloaded conversion (IEC TO_STRING): dispatch on value type.
        "TO_STRING" => unary(args, to_string_value),
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

/// Extensible IEC `CONCAT`: join all STRING arguments. Any non-STRING argument
/// yields `Unknown`.
fn concat(args: &[Value]) -> Value {
    let mut out = String::new();
    for arg in args {
        match arg {
            Value::Str(part) => out.push_str(part),
            _ => return Value::Unknown,
        }
    }
    Value::Str(out)
}

fn expt(base: &Value, exponent: &Value) -> Value {
    match (to_f64(base), to_f64(exponent)) {
        (Some(base), Some(exponent)) => Value::Real(base.powf(exponent)),
        _ => Value::Unknown,
    }
}

fn shl(value: &Value, shift: &Value) -> Value {
    match (value, shift) {
        (Value::Int(bits), Value::Int(by)) if (0..64).contains(by) => {
            Value::Int(((*bits as u64) << *by) as i64)
        }
        _ => Value::Unknown,
    }
}

fn shr(value: &Value, shift: &Value) -> Value {
    match (value, shift) {
        // Logical (unsigned) right shift, matching bit-string semantics.
        (Value::Int(bits), Value::Int(by)) if (0..64).contains(by) => {
            Value::Int(((*bits as u64) >> *by) as i64)
        }
        _ => Value::Unknown,
    }
}

fn left(value: &Value, count: &Value) -> Value {
    match (value, count) {
        (Value::Str(text), Value::Int(count)) if *count >= 0 => {
            Value::Str(text.chars().take(*count as usize).collect())
        }
        _ => Value::Unknown,
    }
}

fn right(value: &Value, count: &Value) -> Value {
    match (value, count) {
        (Value::Str(text), Value::Int(count)) if *count >= 0 => {
            let total = text.chars().count();
            let skip = total.saturating_sub(*count as usize);
            Value::Str(text.chars().skip(skip).collect())
        }
        _ => Value::Unknown,
    }
}

/// IEC `MID(IN, L, P)`: `L` characters from 1-based position `P`.
fn mid(args: &[Value]) -> Value {
    match args {
        [Value::Str(text), Value::Int(length), Value::Int(position)]
            if *length >= 0 && *position >= 1 =>
        {
            Value::Str(
                text.chars()
                    .skip((*position - 1) as usize)
                    .take(*length as usize)
                    .collect(),
            )
        }
        _ => Value::Unknown,
    }
}

fn real_to_string_value(value: &Value) -> Value {
    match value {
        Value::Real(real) => Value::Str(real_to_string(*real)),
        Value::Int(int) => Value::Str(real_to_string(*int as f64)),
        _ => Value::Unknown,
    }
}

fn bool_to_string(value: &Value) -> Value {
    match value {
        Value::Bool(flag) => Value::Str(if *flag { "TRUE" } else { "FALSE" }.to_owned()),
        _ => Value::Unknown,
    }
}

fn time_to_string_value(value: &Value) -> Value {
    match value {
        Value::Time(ms) => Value::Str(time_to_string(*ms)),
        _ => Value::Unknown,
    }
}

/// IEC `TO_STRING`: overloaded conversion that renders any elementary value
/// using the same formatting as its typed `*_TO_STRING` counterpart.
fn to_string_value(value: &Value) -> Value {
    match value {
        Value::Bool(flag) => Value::Str(if *flag { "TRUE" } else { "FALSE" }.to_owned()),
        Value::Int(int) => Value::Str(int.to_string()),
        Value::Real(real) => Value::Str(real_to_string(*real)),
        Value::Time(ms) => Value::Str(time_to_string(*ms)),
        Value::Str(text) => Value::Str(text.clone()),
        Value::Unknown => Value::Unknown,
    }
}

fn to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Real(real) => Some(*real),
        Value::Int(int) => Some(*int as f64),
        _ => None,
    }
}

/// Render a REAL the way CODESYS/TwinCAT `REAL_TO_STRING` does: a decimal form
/// that always keeps the point and a trailing zero for whole numbers (`12.0`,
/// `1024.0`) and trims to at most six fractional digits otherwise (`3.5`).
pub fn real_to_string(value: f64) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    if value == value.trunc() && value.abs() < 1e15 {
        return format!("{value:.1}");
    }
    let rendered = format!("{value:.6}");
    let trimmed = rendered.trim_end_matches('0');
    if trimmed.ends_with('.') {
        format!("{trimmed}0")
    } else {
        trimmed.to_owned()
    }
}

/// Render a duration (milliseconds) as an IEC `T#` literal in canonical
/// compound form (`T#0ms`, `T#2s`, `T#1h30m`).
pub fn time_to_string(ms: i64) -> String {
    if ms == 0 {
        return "T#0ms".to_owned();
    }
    let negative = ms < 0;
    let mut remaining = ms.unsigned_abs();
    let mut out = String::from("T#");
    if negative {
        out.push('-');
    }
    for (unit_ms, suffix) in [
        (86_400_000u64, "d"),
        (3_600_000, "h"),
        (60_000, "m"),
        (1_000, "s"),
        (1, "ms"),
    ] {
        let amount = remaining / unit_ms;
        if amount > 0 {
            out.push_str(&amount.to_string());
            out.push_str(suffix);
            remaining %= unit_ms;
        }
    }
    out
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
