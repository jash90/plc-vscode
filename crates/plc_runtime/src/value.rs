use std::fmt;

/// Runtime value held in the variable table.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Real(f64),
    Str(String),
    /// Time/duration in milliseconds.
    Time(i64),
    /// Unresolved or uninitialized value.
    Unknown,
}

impl Value {
    /// Parse a Structured Text literal into a runtime value, if recognized.
    ///
    /// Handles booleans, single-quoted strings, duration literals (`T#1h30m`,
    /// `T#2s`, `T#100ms`), radix/base literals (`16#FF`, `2#1010`, `8#17`),
    /// IEC typed-literal prefixes (`WORD#16#0F`, `INT#42`, `BOOL#1`,
    /// `LREAL#3.14`), and decimal integer/real literals with `_` separators.
    pub fn parse_literal(token: &str) -> Option<Self> {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            return None;
        }

        let upper = trimmed.to_ascii_uppercase();
        if upper == "TRUE" {
            return Some(Value::Bool(true));
        }
        if upper == "FALSE" {
            return Some(Value::Bool(false));
        }
        if trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2 {
            return Some(Value::Str(trimmed[1..trimmed.len() - 1].to_owned()));
        }

        // Duration literals: `T#`, `TIME#`, `LT#`, `LTIME#`.
        for prefix in ["LTIME#", "LT#", "TIME#", "T#"] {
            if let Some(rest) = upper.strip_prefix(prefix) {
                return parse_duration_ms(rest).map(Value::Time);
            }
        }

        Self::parse_numeric(&upper)
    }

    /// Parse a numeric literal (already uppercased): a radix literal
    /// (`base#digits`), an IEC typed prefix (`TYPE#value`), or a decimal
    /// integer/real, all allowing `_` digit separators.
    fn parse_numeric(upper: &str) -> Option<Self> {
        let cleaned = upper.replace('_', "");

        if let Some((head, rest)) = cleaned.split_once('#') {
            // `base#digits` (radix) when the head is a plain number, otherwise a
            // typed literal prefix (`WORD#...`, `INT#...`, `BOOL#...`).
            if let Ok(radix) = head.parse::<u32>() {
                if (2..=16).contains(&radix) {
                    return i64::from_str_radix(rest, radix).ok().map(Value::Int);
                }
                return None;
            }
            return match head {
                "BOOL" => match rest {
                    "1" | "TRUE" => Some(Value::Bool(true)),
                    "0" | "FALSE" => Some(Value::Bool(false)),
                    _ => None,
                },
                "REAL" | "LREAL" => Self::parse_numeric(rest).map(|value| match value {
                    Value::Int(int) => Value::Real(int as f64),
                    other => other,
                }),
                _ => Self::parse_numeric(rest),
            };
        }

        if let Ok(int) = cleaned.parse::<i64>() {
            return Some(Value::Int(int));
        }
        if let Ok(real) = cleaned.parse::<f64>() {
            return Some(Value::Real(real));
        }
        None
    }

    /// Cold-start default value for a declared IEC type name.
    pub fn type_default(type_name: &str) -> Value {
        match type_name.trim().to_ascii_uppercase().as_str() {
            "BOOL" => Value::Bool(false),
            "SINT" | "INT" | "DINT" | "LINT" | "USINT" | "UINT" | "UDINT" | "ULINT" => {
                Value::Int(0)
            }
            // Bit-string types (BYTE/WORD/DWORD/LWORD) are modeled as integers
            // so bitwise operators and `*_TO_STRING` produce decimal values.
            "BYTE" | "WORD" | "DWORD" | "LWORD" => Value::Int(0),
            "REAL" | "LREAL" => Value::Real(0.0),
            "STRING" | "WSTRING" => Value::Str(String::new()),
            "TIME" | "DATE" | "TIME_OF_DAY" | "TOD" | "DATE_AND_TIME" | "DT" => Value::Time(0),
            _ => Value::Unknown,
        }
    }

    // Named operations over two operands; not the `std::ops` traits.
    #[allow(clippy::should_implement_trait)]
    pub fn add(left: Value, right: Value) -> Value {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => Value::Int(a + b),
            (Value::Real(a), Value::Real(b)) => Value::Real(a + b),
            (Value::Real(a), Value::Int(b)) => Value::Real(a + b as f64),
            (Value::Int(a), Value::Real(b)) => Value::Real(a as f64 + b),
            (Value::Time(a), Value::Time(b)) => Value::Time(a + b),
            _ => Value::Unknown,
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn sub(left: Value, right: Value) -> Value {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => Value::Int(a - b),
            (Value::Real(a), Value::Real(b)) => Value::Real(a - b),
            (Value::Real(a), Value::Int(b)) => Value::Real(a - b as f64),
            (Value::Int(a), Value::Real(b)) => Value::Real(a as f64 - b),
            (Value::Time(a), Value::Time(b)) => Value::Time(a - b),
            _ => Value::Unknown,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Bool(value) => write!(formatter, "{}", if *value { "TRUE" } else { "FALSE" }),
            Value::Int(value) => write!(formatter, "{value}"),
            Value::Real(value) => write!(formatter, "{value}"),
            Value::Str(value) => write!(formatter, "'{value}'"),
            Value::Time(value) => write!(formatter, "T#{value}ms"),
            Value::Unknown => write!(formatter, "<unknown>"),
        }
    }
}

/// Parse an IEC duration body (the part after `T#`) into milliseconds.
///
/// Supports compound, ordered unit groups (`1h30m`, `2s500ms`, `1d2h3m4s5ms`)
/// with units `d`, `h`, `m` (minutes), `s`, and `ms`; each group may be
/// fractional (`1.5s`). A bare trailing number with no unit is treated as
/// milliseconds. Returns `None` for an empty or malformed body.
fn parse_duration_ms(rest: &str) -> Option<i64> {
    let body = rest.trim().trim_start_matches('-').replace('_', "");
    if body.is_empty() {
        return None;
    }
    let negative = rest.trim().starts_with('-');
    let bytes = body.as_bytes();
    let mut cursor = 0usize;
    let mut total_ms = 0f64;
    let mut matched = false;

    while cursor < bytes.len() {
        let number_start = cursor;
        while cursor < bytes.len() && (bytes[cursor].is_ascii_digit() || bytes[cursor] == b'.') {
            cursor += 1;
        }
        if cursor == number_start {
            return None;
        }
        let number: f64 = body[number_start..cursor].parse().ok()?;

        let unit_start = cursor;
        while cursor < bytes.len() && bytes[cursor].is_ascii_alphabetic() {
            cursor += 1;
        }
        let multiplier = match &body[unit_start..cursor] {
            "D" => 86_400_000f64,
            "H" => 3_600_000f64,
            "M" => 60_000f64,
            "S" => 1_000f64,
            "MS" => 1f64,
            // A bare number with no unit is milliseconds.
            "" => 1f64,
            _ => return None,
        };
        total_ms += number * multiplier;
        matched = true;
    }

    if !matched {
        return None;
    }
    let total = total_ms.round() as i64;
    Some(if negative { -total } else { total })
}
