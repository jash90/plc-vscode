//! Shared compiler-core API for PLC VS Code.
//!
//! This crate defines the stable contract consumed by the CLI, LSP server,
//! runtime, bytecode VM, and native backend. The implementation keeps syntax
//! checks behind the `plc_syntax` crate and semantic checks behind the
//! `plc_semantics` crate so all consumers share the same compiler boundary.

mod execution;

use execution::collect_execution_output;
use plc_semantics::{Symbol as SemanticSymbol, SymbolKind as SemanticSymbolKind, analyze_file};
use plc_syntax::{TextRange, TokenKind, parse_source};

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

/// Stable symbol kind exposed by compiler-core to IDE consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Program,
    Function,
    FunctionBlock,
    Action,
    Variable,
    Type,
    Keyword,
}

/// Hierarchical document symbol used by LSP and future editor consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSymbol {
    pub name: String,
    pub detail: Option<String>,
    pub kind: SymbolKind,
    pub range: Range,
    pub selection_range: Range,
    pub children: Vec<DocumentSymbol>,
}

/// Document symbol analysis result for a source snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolAnalysis {
    uri: String,
    version: i32,
    symbols: Vec<DocumentSymbol>,
}

impl SymbolAnalysis {
    pub fn uri(&self) -> &str {
        &self.uri
    }

    pub fn version(&self) -> i32 {
        self.version
    }

    pub fn symbols(&self) -> &[DocumentSymbol] {
        &self.symbols
    }
}

/// Completion candidate exposed by compiler-core to LSP and editor consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionCandidate {
    pub label: String,
    pub detail: Option<String>,
    pub kind: SymbolKind,
}

/// Hover payload exposed by compiler-core to LSP and editor consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoverInfo {
    pub contents: String,
    pub range: Range,
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

    pub fn document_symbols(&self, document: &SourceDocument) -> SymbolAnalysis {
        SymbolAnalysis {
            uri: document.uri().to_owned(),
            version: document.version(),
            symbols: document_symbols(document),
        }
    }

    pub fn completions(&self, document: &SourceDocument) -> Vec<CompletionCandidate> {
        completion_candidates(document)
    }

    pub fn hover(&self, document: &SourceDocument, position: Position) -> Option<HoverInfo> {
        hover_at_position(document, position)
    }
}

fn document_symbols(document: &SourceDocument) -> Vec<DocumentSymbol> {
    let semantic = analyze_file(document.uri(), document.text());
    let mut symbols = Vec::new();

    for symbol in semantic
        .symbol_index
        .symbols()
        .iter()
        .filter(|symbol| symbol.container.is_none())
    {
        let children = semantic
            .symbol_index
            .symbols()
            .iter()
            .filter(|child| {
                child
                    .container
                    .as_deref()
                    .is_some_and(|container| container.eq_ignore_ascii_case(&symbol.name))
            })
            .map(|child| DocumentSymbol {
                name: child.name.clone(),
                detail: child
                    .type_kind
                    .as_ref()
                    .map(|type_kind| type_kind.display_name().to_owned()),
                kind: symbol_kind(child.kind),
                range: text_range_to_range(document.text(), child.range),
                selection_range: text_range_to_range(document.text(), child.range),
                children: Vec::new(),
            })
            .collect();

        symbols.push(DocumentSymbol {
            name: symbol.name.clone(),
            detail: None,
            kind: symbol_kind(symbol.kind),
            range: text_range_to_range(document.text(), symbol.range),
            selection_range: text_range_to_range(document.text(), symbol.range),
            children,
        });
    }

    symbols
}

fn symbol_kind(kind: SemanticSymbolKind) -> SymbolKind {
    match kind {
        SemanticSymbolKind::Program => SymbolKind::Program,
        SemanticSymbolKind::Function => SymbolKind::Function,
        SemanticSymbolKind::FunctionBlock => SymbolKind::FunctionBlock,
        SemanticSymbolKind::Action => SymbolKind::Action,
        SemanticSymbolKind::Variable => SymbolKind::Variable,
        SemanticSymbolKind::Type => SymbolKind::Type,
    }
}

fn completion_candidates(document: &SourceDocument) -> Vec<CompletionCandidate> {
    let semantic = analyze_file(document.uri(), document.text());
    let mut candidates: Vec<CompletionCandidate> = semantic
        .symbol_index
        .symbols()
        .iter()
        .map(|symbol| CompletionCandidate {
            label: symbol.name.clone(),
            detail: symbol_detail(symbol),
            kind: symbol_kind(symbol.kind),
        })
        .collect();

    candidates.extend(ST_KEYWORDS.iter().map(|keyword| CompletionCandidate {
        label: (*keyword).to_owned(),
        detail: Some("Structured Text keyword".to_owned()),
        kind: SymbolKind::Keyword,
    }));
    candidates.sort_by(|left, right| left.label.cmp(&right.label));
    candidates.dedup_by(|left, right| left.label.eq_ignore_ascii_case(&right.label));
    candidates
}

fn hover_at_position(document: &SourceDocument, position: Position) -> Option<HoverInfo> {
    let offset = position_to_byte_offset(document.text(), position)?;
    let parse = parse_source(document.text());
    let token = parse
        .tokens()
        .iter()
        .find(|token| token.range.start <= offset && offset <= token.range.end)?;

    if token.kind == TokenKind::Keyword {
        return Some(HoverInfo {
            contents: format!("Structured Text keyword `{}`", token.text),
            range: text_range_to_range(document.text(), token.range),
        });
    }

    if token.kind != TokenKind::Identifier {
        return None;
    }

    let semantic = analyze_file(document.uri(), document.text());
    let symbol = semantic
        .symbol_index
        .symbols()
        .iter()
        .find(|symbol| symbol.name.eq_ignore_ascii_case(&token.text))?;

    Some(HoverInfo {
        contents: symbol_hover_contents(symbol),
        range: text_range_to_range(document.text(), token.range),
    })
}

fn symbol_detail(symbol: &SemanticSymbol) -> Option<String> {
    symbol
        .type_kind
        .as_ref()
        .map(|type_kind| type_kind.display_name().to_owned())
        .or_else(|| Some(format!("{:?}", symbol.kind)))
}

fn symbol_hover_contents(symbol: &SemanticSymbol) -> String {
    if let Some(type_kind) = symbol.type_kind.as_ref() {
        format!("{}: {}", symbol.name, type_kind.display_name())
    } else {
        format!("{} ({:?})", symbol.name, symbol.kind)
    }
}

const ST_KEYWORDS: &[&str] = &[
    "ACTION",
    "CASE",
    "END_ACTION",
    "END_CASE",
    "END_FOR",
    "END_FUNCTION",
    "END_FUNCTION_BLOCK",
    "END_IF",
    "END_PROGRAM",
    "END_REPEAT",
    "END_VAR",
    "END_WHILE",
    "EXIT",
    "FOR",
    "FUNCTION",
    "FUNCTION_BLOCK",
    "IF",
    "PROGRAM",
    "REPEAT",
    "RETURN",
    "THEN",
    "VAR",
    "VAR_GLOBAL",
    "VAR_IN_OUT",
    "VAR_INPUT",
    "VAR_OUTPUT",
    "VAR_TEMP",
    "WHILE",
];

fn position_to_byte_offset(text: &str, position: Position) -> Option<usize> {
    let mut line = 0u32;
    let mut character = 0u32;

    for (idx, ch) in text.char_indices() {
        if line == position.line && character == position.character {
            return Some(idx);
        }
        if ch == '\n' {
            line += 1;
            character = 0;
            if line == position.line && position.character == 0 {
                return Some(idx + ch.len_utf8());
            }
        } else {
            character += 1;
        }
    }

    if line == position.line && character == position.character {
        Some(text.len())
    } else {
        None
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
