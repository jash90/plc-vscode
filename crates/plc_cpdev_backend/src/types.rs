//! CPDev scalar types: the in-memory byte size used for data-segment layout and
//! `MCD` immediates, and IEC name resolution.
//!
//! Note: a typed vmcode's low nibble is *usually* the type code (e.g. `EQ_INT`
//! = `0x1202`), but some families (`NOT`/`NEG`, conversions) sub-enumerate it
//! irregularly (`NOT_BYTE` = `0x0511`, not `…5`). So the backend never computes a
//! vmcode from a type nibble — it looks the variant up in the parsed
//! [`spec`](crate::spec) table by `(name, operand type)`. This enum therefore
//! carries only what layout/encoding need.

/// A CPDev scalar type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpType {
    Bool,
    Sint,
    Int,
    Dint,
    Lint,
    Byte,
    Word,
    Dword,
    Lword,
    Real,
    Lreal,
    Time,
    Date,
    Tod,
    Dt,
    /// STRING with the given character capacity (`chars_size`). The on-data image
    /// is `[length:1][chars_size:1][padding:2][chars: capacity]`.
    Str(u16),
}

/// Default character capacity for an unsized `STRING` declaration.
pub const DEFAULT_STR_CAP: u16 = 80;

impl CpType {
    /// In-memory byte size of a scalar. `STRING` is sized by capacity elsewhere
    /// (returns 0 here as a sentinel).
    pub fn size(self) -> usize {
        match self {
            CpType::Bool | CpType::Sint | CpType::Byte => 1,
            CpType::Int | CpType::Word => 2,
            CpType::Dint | CpType::Dword | CpType::Real | CpType::Time | CpType::Date => 4,
            CpType::Lint | CpType::Lword | CpType::Lreal | CpType::Dt | CpType::Tod => 8,
            // 4-byte header ([length][chars_size][padding:2]) + inline characters.
            CpType::Str(cap) => 4 + cap as usize,
        }
    }

    /// The canonical IEC type name (as it appears in spec `type=` attributes and
    /// `.DCP` `Type=` attributes).
    pub fn iec_name(self) -> &'static str {
        match self {
            CpType::Bool => "BOOL",
            CpType::Sint => "SINT",
            CpType::Int => "INT",
            CpType::Dint => "DINT",
            CpType::Lint => "LINT",
            CpType::Byte => "BYTE",
            CpType::Word => "WORD",
            CpType::Dword => "DWORD",
            CpType::Lword => "LWORD",
            CpType::Real => "REAL",
            CpType::Lreal => "LREAL",
            CpType::Time => "TIME",
            CpType::Date => "DATE",
            CpType::Tod => "TIME_OF_DAY",
            CpType::Dt => "DATE_AND_TIME",
            CpType::Str(_) => "STRING",
        }
    }

    /// Resolve an IEC type name (including the unsigned aliases) to a [`CpType`].
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name.trim().to_ascii_uppercase().as_str() {
            "BOOL" => CpType::Bool,
            "SINT" => CpType::Sint,
            "INT" => CpType::Int,
            "DINT" => CpType::Dint,
            "LINT" => CpType::Lint,
            "BYTE" | "USINT" => CpType::Byte,
            "WORD" | "UINT" => CpType::Word,
            "DWORD" | "UDINT" => CpType::Dword,
            "LWORD" | "ULINT" => CpType::Lword,
            "REAL" => CpType::Real,
            "LREAL" => CpType::Lreal,
            "TIME" => CpType::Time,
            "DATE" => CpType::Date,
            "TIME_OF_DAY" | "TOD" => CpType::Tod,
            "DATE_AND_TIME" | "DT" => CpType::Dt,
            "STRING" | "WSTRING" => CpType::Str(DEFAULT_STR_CAP),
            _ => return None,
        })
    }
}
