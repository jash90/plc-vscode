//! Shared compiler-core API for PLC VS Code.
//!
//! This crate defines the stable contract consumed by the CLI, LSP server,
//! runtime, bytecode VM, and native backend. The implementation keeps syntax
//! checks behind the `plc_syntax` crate and semantic checks behind the
//! `plc_semantics` crate so all consumers share the same compiler boundary.

mod execution;

use execution::collect_execution_output;
use plc_syntax::{TextRange, parse_source};

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
    let parse = parse_source(text);
    let syntax_diagnostics: Vec<Diagnostic> = parse
        .diagnostics()
        .iter()
        .map(|diagnostic| Diagnostic {
            severity: DiagnosticSeverity::Error,
            range: text_range_to_range(text, diagnostic.range),
            code: diagnostic.code,
            message: diagnostic.message.clone(),
        })
        .collect();

    // An unclosed block comment makes the remainder of the file lexical trivia,
    // so suppress follow-on parser terminator diagnostics to keep the primary
    // error stable for CLI and LSP consumers.
    if syntax_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "PLC0001")
    {
        return syntax_diagnostics
            .into_iter()
            .filter(|diagnostic| diagnostic.code == "PLC0001")
            .collect();
    }

    if !syntax_diagnostics.is_empty() {
        return syntax_diagnostics;
    }

    let mut diagnostics = syntax_diagnostics;
    diagnostics.extend(
        plc_semantics::analyze_file("memory://document", text)
            .diagnostics
            .into_iter()
            .map(|diagnostic| Diagnostic {
                severity: DiagnosticSeverity::Error,
                range: text_range_to_range(text, diagnostic.range),
                code: diagnostic.code,
                message: diagnostic.message,
            }),
    );
    diagnostics
}

fn text_range_to_range(text: &str, range: TextRange) -> Range {
    Range {
        start: byte_offset_to_position(text, range.start),
        end: byte_offset_to_position(text, range.end.max(range.start + 1).min(text.len())),
    }
}

fn byte_offset_to_position(text: &str, offset: usize) -> Position {
    let mut line = 0u32;
    let mut character = 0u32;

    for (idx, ch) in text.char_indices() {
        if idx >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            character += 1;
        }
    }

    Position { line, character }
}
