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
        if let Some(rest) = upper.strip_prefix("T#") {
            return parse_duration_ms(rest).map(Value::Time);
        }
        if let Ok(int) = trimmed.parse::<i64>() {
            return Some(Value::Int(int));
        }
        if let Ok(real) = trimmed.parse::<f64>() {
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

/// Parse a simple `<n>ms` / `<n>s` duration suffix into milliseconds.
fn parse_duration_ms(rest: &str) -> Option<i64> {
    if let Some(ms) = rest.strip_suffix("MS") {
        return ms.trim().parse::<i64>().ok();
    }
    if let Some(secs) = rest.strip_suffix('S') {
        return secs.trim().parse::<i64>().ok().map(|s| s * 1000);
    }
    rest.trim().parse::<i64>().ok()
}
