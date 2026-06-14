use plc_syntax::TextRange;

/// Source file snapshot analyzed by the semantic layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub uri: String,
    pub text: String,
}

impl SourceFile {
    pub fn new(uri: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            text: text.into(),
        }
    }
}

/// Semantic diagnostic with byte range for compiler-core mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticDiagnostic {
    pub code: &'static str,
    pub range: TextRange,
    pub message: String,
}

/// IEC elementary and derived type model used by early diagnostics and LSP
/// hover.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeKind {
    Bool,
    Integer,
    Real,
    Time,
    String,
    WString,
    Array,
    Struct,
    Enum,
    Alias,
    Subrange,
    Derived(String),
    Unknown(String),
}

impl TypeKind {
    pub fn from_name(name: &str) -> Self {
        let upper = name.to_ascii_uppercase();
        match upper.as_str() {
            "BOOL" => Self::Bool,
            "SINT" | "INT" | "DINT" | "LINT" | "USINT" | "UINT" | "UDINT" | "ULINT" => {
                Self::Integer
            }
            "REAL" | "LREAL" => Self::Real,
            "TIME" | "DATE" | "TIME_OF_DAY" | "TOD" | "DATE_AND_TIME" | "DT" => Self::Time,
            "STRING" => Self::String,
            "WSTRING" => Self::WString,
            "ARRAY" => Self::Array,
            "STRUCT" => Self::Struct,
            "ENUM" => Self::Enum,
            "ALIAS" => Self::Alias,
            "SUBRANGE" => Self::Subrange,
            _ if !name.trim().is_empty() => Self::Derived(name.to_owned()),
            _ => Self::Unknown(name.to_owned()),
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            Self::Bool => "BOOL",
            Self::Integer => "integer",
            Self::Real => "real",
            Self::Time => "time/date",
            Self::String => "STRING",
            Self::WString => "WSTRING",
            Self::Array => "ARRAY",
            Self::Struct => "STRUCT",
            Self::Enum => "ENUM",
            Self::Alias => "ALIAS",
            Self::Subrange => "SUBRANGE",
            Self::Derived(name) | Self::Unknown(name) => name.as_str(),
        }
    }

    pub fn assignment_compatible(&self, value: &Self) -> bool {
        matches!(
            (self, value),
            (Self::Integer, Self::Integer)
                | (Self::Real, Self::Real)
                | (Self::Real, Self::Integer)
                | (Self::Bool, Self::Bool)
                | (Self::String, Self::String)
                | (Self::WString, Self::String)
                | (Self::Time, Self::Time)
        ) || matches!(value, Self::Unknown(_))
    }
}

/// Indexed symbol category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Program,
    Function,
    FunctionBlock,
    Action,
    Variable,
    Type,
}

/// Symbol indexed from syntax output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub type_kind: Option<TypeKind>,
    pub uri: String,
    pub range: TextRange,
    pub container: Option<String>,
}

/// Workspace symbol index.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolIndex {
    symbols: Vec<Symbol>,
}

impl SymbolIndex {
    pub fn insert(&mut self, symbol: Symbol) {
        self.symbols.push(symbol);
    }

    pub fn symbols(&self) -> &[Symbol] {
        &self.symbols
    }

    pub fn find_in_container(&self, container: &str, name: &str) -> Option<&Symbol> {
        self.symbols.iter().find(|symbol| {
            symbol.name.eq_ignore_ascii_case(name)
                && symbol
                    .container
                    .as_deref()
                    .is_some_and(|scope| scope.eq_ignore_ascii_case(container))
        })
    }

    pub fn find_top_level(&self, name: &str) -> Option<&Symbol> {
        self.symbols
            .iter()
            .find(|symbol| symbol.name.eq_ignore_ascii_case(name) && symbol.container.is_none())
    }
}

/// Semantic analysis result for one or more files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticAnalysis {
    pub symbol_index: SymbolIndex,
    pub diagnostics: Vec<SemanticDiagnostic>,
}
