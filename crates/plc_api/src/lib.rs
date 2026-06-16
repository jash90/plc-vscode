//! Backend-agnostic ports and shared data types for PLC VS Code.
//!
//! This crate is the stable seam between the IDE/CLI **frontends** and the
//! analysis/compiler **backends**. It has zero `plc_*` dependencies so both the
//! language-service side (`plc_compiler_core`) and the runtime side
//! (`plc_runtime`) can depend on it without a cycle, and so the LSP build never
//! pulls in the runtime.
//!
//! Two ports define the plug-in surface:
//!
//! - [`LanguageService`] — the analysis/IDE backend a [tower-lsp] server (or any
//!   other frontend) consumes. Implement it to bring your own analyzer; the
//!   provided `plc_lsp_server` can hold any `Arc<dyn LanguageService>`.
//! - [`ExecutionEngine`] — the compile-and-run backend the CLI drives.
//!   Implement it to bring your own compiler/runtime (interpreter, bytecode VM,
//!   LLVM JIT, remote PLC, …).
//!
//! Both traits are intentionally **object-safe** (all methods take `&self` /
//! `&mut self` plus borrowed/`Copy` arguments and return owned values — no
//! generics, no associated types, no `async`, no `Self` by value), so they can
//! be used as `dyn` trait objects. Keep them that way.

// ---------------------------------------------------------------------------
// Documents and results
// ---------------------------------------------------------------------------

/// Source document snapshot passed into backend operations.
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

/// Analysis result for a single source document snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Analysis {
    uri: String,
    version: i32,
    diagnostics: Vec<Diagnostic>,
}

impl Analysis {
    /// Construct an analysis result (used by backends across the crate boundary).
    pub fn new(uri: impl Into<String>, version: i32, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            uri: uri.into(),
            version,
            diagnostics,
        }
    }

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

/// Result of executing a Structured Text document with a development runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionResult {
    diagnostics: Vec<Diagnostic>,
    output: Vec<String>,
}

impl ExecutionResult {
    /// Construct an execution result (used by backends across the crate boundary).
    pub fn new(diagnostics: Vec<Diagnostic>, output: Vec<String>) -> Self {
        Self {
            diagnostics,
            output,
        }
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn output(&self) -> &[String] {
        &self.output
    }
}

// ---------------------------------------------------------------------------
// Diagnostics, positions, ranges
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Symbols
// ---------------------------------------------------------------------------

/// Stable symbol kind exposed to IDE consumers.
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
    /// Construct a symbol analysis (used by backends across the crate boundary).
    pub fn new(uri: impl Into<String>, version: i32, symbols: Vec<DocumentSymbol>) -> Self {
        Self {
            uri: uri.into(),
            version,
            symbols,
        }
    }

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

/// Flat workspace symbol exposed for `workspace/symbol`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub location: Location,
    pub container_name: Option<String>,
}

// ---------------------------------------------------------------------------
// Completion, hover, signature help, navigation
// ---------------------------------------------------------------------------

/// Completion candidate exposed to LSP and editor consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionCandidate {
    pub label: String,
    pub detail: Option<String>,
    pub kind: SymbolKind,
}

/// Hover payload exposed to LSP and editor consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoverInfo {
    pub contents: String,
    pub range: Range,
}

/// A single parameter of a call signature exposed to IDE consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterInfo {
    pub label: String,
}

/// Call signature payload for signature help.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureInfo {
    pub label: String,
    pub parameters: Vec<ParameterInfo>,
    pub active_parameter: Option<u32>,
}

/// Source location (document URI + range) used for navigation features.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub uri: String,
    pub range: Range,
}

// ---------------------------------------------------------------------------
// Semantic tokens, edits, code actions
// ---------------------------------------------------------------------------

/// Semantic token category exposed for syntax highlighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticTokenKind {
    Keyword,
    Type,
    Variable,
    Function,
    FunctionBlock,
    Number,
    String,
    Comment,
    Operator,
}

/// A classified, single-line semantic token with its source range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticToken {
    pub range: Range,
    pub kind: SemanticTokenKind,
}

/// Text edit (range replacement) used by formatting and code actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub range: Range,
    pub new_text: String,
}

/// Code action (quick fix) with a title and the edits it applies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeAction {
    pub title: String,
    pub edits: Vec<TextEdit>,
}

// ---------------------------------------------------------------------------
// Ports
// ---------------------------------------------------------------------------

/// The pluggable analysis / IDE backend port.
///
/// A frontend (e.g. the provided `plc_lsp_server`) holds a
/// `dyn LanguageService` and never names a concrete analyzer, so a third party
/// can supply their own implementation. `plc_compiler_core::CompilerCore` is
/// the default implementation.
///
/// Object-safety must be preserved: do not add generic methods, associated
/// types, `async fn`, or `Self`-by-value receivers.
pub trait LanguageService {
    fn analyze(&self, document: &SourceDocument) -> Analysis;
    /// Lightweight, runtime-free execution preview. For the real
    /// compile-and-run path use [`ExecutionEngine`].
    fn execute(&self, document: &SourceDocument) -> ExecutionResult;
    fn document_symbols(&self, document: &SourceDocument) -> SymbolAnalysis;
    fn workspace_symbols(&self, documents: &[SourceDocument], query: &str) -> Vec<WorkspaceSymbol>;
    fn semantic_tokens(&self, document: &SourceDocument) -> Vec<SemanticToken>;
    fn completions(
        &self,
        document: &SourceDocument,
        position: Position,
    ) -> Vec<CompletionCandidate>;
    fn hover(&self, document: &SourceDocument, position: Position) -> Option<HoverInfo>;
    fn signature_help(
        &self,
        document: &SourceDocument,
        position: Position,
    ) -> Option<SignatureInfo>;
    fn definition(&self, document: &SourceDocument, position: Position) -> Option<Location>;
    fn references(
        &self,
        document: &SourceDocument,
        position: Position,
        include_declaration: bool,
    ) -> Vec<Location>;
    fn formatting(&self, document: &SourceDocument) -> Vec<TextEdit>;
    fn formatting_range(&self, document: &SourceDocument, range: Range) -> Vec<TextEdit>;
    fn code_actions(&self, document: &SourceDocument) -> Vec<CodeAction>;
}

/// The pluggable compile-and-run backend port.
///
/// Models what a CLI/host does to execute a program: load it, configure the
/// scan interval, run scan cycles, optionally stage inputs, and read back an
/// online "watch" snapshot (`name = value` lines). Implement it to plug in your
/// own compiler/runtime; `plc_runtime::ScanRuntimeEngine` is the default
/// implementation.
///
/// Values are exchanged as strings to keep this crate dependency-free; a typed
/// value model can be added later as a defaulted method without breaking the
/// port.
pub trait ExecutionEngine {
    /// Load/compile a document. Return build diagnostics on failure. The
    /// reference engine assumes the caller already ran the [`LanguageService`]
    /// diagnostics gate and returns `Ok(())`.
    fn load(&mut self, document: &SourceDocument) -> Result<(), Vec<Diagnostic>>;
    /// Load a pre-built artifact (e.g. compiled bytecode) supplied as raw bytes,
    /// with `uri` identifying its origin (so an engine can locate sidecar files).
    ///
    /// Defaulted to "unsupported" so existing engines need no change: only an
    /// engine that consumes prebuilt binaries (e.g. a bytecode VM loading a
    /// compiled image) overrides this. Object-safe like the rest of the port.
    fn load_artifact(&mut self, bytes: &[u8], uri: &str) -> Result<(), Vec<Diagnostic>> {
        let _ = (bytes, uri);
        Err(vec![Diagnostic {
            severity: DiagnosticSeverity::Error,
            range: Range::at_start(),
            code: "E_ARTIFACT_UNSUPPORTED",
            message: "this execution engine does not support loading prebuilt artifacts".to_owned(),
        }])
    }
    fn set_scan_interval_ms(&mut self, scan_interval_ms: i64);
    fn run_scans(&mut self, cycles: u64);
    /// Stage an input value (parsed from its textual form) for the next scan.
    fn set_input(&mut self, name: &str, value: &str);
    /// Online watch snapshot: `name = value` lines for declared variables.
    fn watch(&self) -> Vec<String>;
}
