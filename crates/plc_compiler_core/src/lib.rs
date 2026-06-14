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
            output: collect_plc_print_output(document.text()),
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

fn collect_plc_print_output(text: &str) -> Vec<String> {
    let mut output = Vec::new();
    let mut remaining = text;
    const CALL: &str = "PLC_PRINT";

    while let Some(call_index) = remaining.to_ascii_uppercase().find(CALL) {
        remaining = &remaining[call_index + CALL.len()..];
        let Some(open_paren) = remaining.find('(') else {
            continue;
        };
        remaining = &remaining[open_paren + 1..];
        let trimmed = remaining.trim_start();
        let Some(quote) = trimmed.chars().next().filter(|c| *c == '\'' || *c == '"') else {
            remaining = trimmed;
            continue;
        };
        let after_quote = &trimmed[quote.len_utf8()..];
        if let Some(end_quote) = after_quote.find(quote) {
            output.push(after_quote[..end_quote].to_owned());
            remaining = &after_quote[end_quote + quote.len_utf8()..];
        } else {
            break;
        }
    }

    output
}
