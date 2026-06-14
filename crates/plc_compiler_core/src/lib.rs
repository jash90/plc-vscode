//! Shared compiler-core API for PLC VS Code.
//!
//! This crate defines the stable contract consumed by the CLI, LSP server,
//! runtime, bytecode VM, and native backend. The first implementation exposes
//! document analysis and diagnostics; later tasks will replace the placeholder
//! checks with the real lexer, parser, semantic model, and backend pipeline.

/// Source document snapshot passed into compiler-core operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDocument {
    uri: String,
    version: i32,
    text: String,
}

impl SourceDocument {
    pub fn new(uri: impl Into<String>, version: i32, text: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            version,
            text: text.into(),
        }
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }

    pub fn version(&self) -> i32 {
        self.version
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

/// Compiler analysis result for a single source document snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Analysis {
    uri: String,
    version: i32,
    diagnostics: Vec<Diagnostic>,
}

impl Analysis {
    pub fn uri(&self) -> &str {
        &self.uri
    }

    pub fn version(&self) -> i32 {
        self.version
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

/// Result of executing a Structured Text document with the development runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionResult {
    diagnostics: Vec<Diagnostic>,
    output: Vec<String>,
}

impl ExecutionResult {
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn output(&self) -> &[String] {
        &self.output
    }
}

/// Diagnostic severity that can be mapped to LSP, CLI, or editor output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

/// Zero-based source position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

/// Half-open source range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

impl Range {
    pub fn at_start() -> Self {
        Self {
            start: Position::default(),
            end: Position {
                line: 0,
                character: 1,
            },
        }
    }
}

/// Compiler diagnostic with stable fields for all consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub range: Range,
    pub code: &'static str,
    pub message: String,
}

/// Shared compiler facade.
#[derive(Debug, Default, Clone, Copy)]
pub struct CompilerCore;

impl CompilerCore {
    pub fn analyze(&self, document: &SourceDocument) -> Analysis {
        let diagnostics = analyze_text(document.text());
        Analysis {
            uri: document.uri().to_owned(),
            version: document.version(),
            diagnostics,
        }
    }

    pub fn execute(&self, document: &SourceDocument) -> ExecutionResult {
        let analysis = self.analyze(document);
        if !analysis.diagnostics().is_empty() {
            return ExecutionResult {
                diagnostics: analysis.diagnostics().to_vec(),
                output: Vec::new(),
            };
        }

        ExecutionResult {
            diagnostics: Vec::new(),
            output: collect_execution_output(document.text()),
        }
    }
}

fn analyze_text(text: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    if has_unclosed_block_comment(text) {
        diagnostics.push(Diagnostic {
            severity: DiagnosticSeverity::Error,
            range: Range::at_start(),
            code: "PLC0001",
            message: "Unclosed block comment: expected closing *)".to_owned(),
        });
        return diagnostics;
    }

    let upper = text.to_ascii_uppercase();
    if upper.contains("PROGRAM") && !upper.contains("END_PROGRAM") {
        diagnostics.push(Diagnostic {
            severity: DiagnosticSeverity::Error,
            range: Range::at_start(),
            code: "PLC0002",
            message: "PROGRAM declaration is missing END_PROGRAM terminator".to_owned(),
        });
    }

    diagnostics
}

fn has_unclosed_block_comment(text: &str) -> bool {
    let mut remaining = text;
    let mut depth = 0usize;

    while let Some(open_or_close) = find_next_comment_marker(remaining) {
        let (idx, marker) = open_or_close;
        remaining = &remaining[idx + 2..];
        match marker {
            "(*" => depth += 1,
            "*)" => depth = depth.saturating_sub(1),
            _ => unreachable!("only known comment markers are returned"),
        }
    }

    depth > 0
}

fn find_next_comment_marker(text: &str) -> Option<(usize, &'static str)> {
    match (text.find("(*"), text.find("*)")) {
        (Some(open), Some(close)) if open < close => Some((open, "(*")),
        (Some(_), Some(close)) => Some((close, "*)")),
        (Some(open), None) => Some((open, "(*")),
        (None, Some(close)) => Some((close, "*)")),
        (None, None) => None,
    }
}

fn collect_execution_output(text: &str) -> Vec<String> {
    collect_initialized_string_variables(text)
}

fn collect_initialized_string_variables(text: &str) -> Vec<String> {
    let mut output = Vec::new();
    let mut in_var_block = false;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        let upper = line.to_ascii_uppercase();

        if upper == "VAR" || upper.starts_with("VAR_") {
            in_var_block = true;
            continue;
        }

        if upper == "END_VAR" {
            in_var_block = false;
            continue;
        }

        if !in_var_block || line.is_empty() || line.starts_with("//") {
            continue;
        }

        if let Some((name, value)) = parse_string_initialization(line) {
            output.push(format!("{name} = {value}"));
        }
    }

    output
}

fn parse_string_initialization(line: &str) -> Option<(String, String)> {
    let (name, rest) = line.split_once(':')?;
    let name = name.trim();
    if name.is_empty() || !is_identifier(name) {
        return None;
    }

    let (type_name, initializer) = rest.split_once(":=")?;
    let type_name = type_name.trim().to_ascii_uppercase();
    if type_name != "STRING" && !type_name.starts_with("STRING[") {
        return None;
    }

    let initializer = initializer.trim().trim_end_matches(';').trim();
    let quote = initializer.chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }

    let after_quote = &initializer[quote.len_utf8()..];
    let end_quote = after_quote.find(quote)?;
    Some((name.to_owned(), after_quote[..end_quote].to_owned()))
}

fn is_identifier(candidate: &str) -> bool {
    let mut chars = candidate.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}
