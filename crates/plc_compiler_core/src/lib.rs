//! Shared compiler-core API for PLC VS Code.
//!
//! This crate defines the stable contract consumed by the CLI, LSP server,
//! runtime, bytecode VM, and native backend. The implementation keeps syntax
//! checks behind the `plc_syntax` crate and semantic checks behind the
//! `plc_semantics` crate so all consumers share the same compiler boundary.

mod execution;

use execution::collect_execution_output;
use plc_semantics::{
    SourceFile, Symbol as SemanticSymbol, SymbolKind as SemanticSymbolKind, analyze_file,
    analyze_workspace,
};
use plc_syntax::{Pou, PouKind, TextRange, Token, TokenKind, VarBlockKind, parse_source};

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

/// A single parameter of a call signature exposed to IDE consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterInfo {
    pub label: String,
}

/// Call signature payload exposed by compiler-core for signature help.
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

/// Flat workspace symbol exposed by compiler-core for `workspace/symbol`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub location: Location,
    pub container_name: Option<String>,
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

    /// Query top-level declarations across all supplied documents. An empty
    /// `query` returns every top-level symbol; otherwise names are matched
    /// case-insensitively as a substring.
    pub fn workspace_symbols(
        &self,
        documents: &[SourceDocument],
        query: &str,
    ) -> Vec<WorkspaceSymbol> {
        workspace_symbols(documents, query)
    }

    pub fn completions(
        &self,
        document: &SourceDocument,
        position: Position,
    ) -> Vec<CompletionCandidate> {
        completion_candidates(document, position)
    }

    pub fn hover(&self, document: &SourceDocument, position: Position) -> Option<HoverInfo> {
        hover_at_position(document, position)
    }

    pub fn signature_help(
        &self,
        document: &SourceDocument,
        position: Position,
    ) -> Option<SignatureInfo> {
        signature_help_at_position(document, position)
    }

    pub fn definition(&self, document: &SourceDocument, position: Position) -> Option<Location> {
        definition_at_position(document, position)
    }

    pub fn references(
        &self,
        document: &SourceDocument,
        position: Position,
        include_declaration: bool,
    ) -> Vec<Location> {
        references_at_position(document, position, include_declaration)
    }

    pub fn formatting(&self, document: &SourceDocument) -> Vec<TextEdit> {
        let formatted = format_text(document.text());
        if formatted == document.text() {
            return Vec::new();
        }
        vec![TextEdit {
            range: whole_document_range(document.text()),
            new_text: formatted,
        }]
    }

    pub fn formatting_range(&self, document: &SourceDocument, range: Range) -> Vec<TextEdit> {
        let text = document.text();
        let lines: Vec<&str> = text.split('\n').collect();
        if lines.is_empty() {
            return Vec::new();
        }
        let last = lines.len() - 1;
        let start_line = (range.start.line as usize).min(last);
        let end_line = (range.end.line as usize).min(last);
        if start_line > end_line {
            return Vec::new();
        }

        let formatted: Vec<String> = lines[start_line..=end_line]
            .iter()
            .map(|line| case_keywords(line).trim_end().to_owned())
            .collect();
        let new_text = formatted.join("\n");

        let original: String = lines[start_line..=end_line].join("\n");
        if new_text == original {
            return Vec::new();
        }

        vec![TextEdit {
            range: Range {
                start: Position {
                    line: start_line as u32,
                    character: 0,
                },
                end: Position {
                    line: end_line as u32,
                    character: lines[end_line].chars().count() as u32,
                },
            },
            new_text,
        }]
    }

    pub fn code_actions(&self, document: &SourceDocument) -> Vec<CodeAction> {
        let mut actions = Vec::new();
        let analysis = self.analyze(document);

        if analysis
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == "PLC0002")
        {
            let end = byte_offset_to_position(document.text(), document.text().len());
            let suffix = if document.text().ends_with('\n') {
                "END_PROGRAM\n"
            } else {
                "\nEND_PROGRAM\n"
            };
            actions.push(CodeAction {
                title: "Add missing END_PROGRAM terminator".to_owned(),
                edits: vec![TextEdit {
                    range: Range { start: end, end },
                    new_text: suffix.to_owned(),
                }],
            });
        }

        actions
    }
}

fn whole_document_range(text: &str) -> Range {
    Range {
        start: Position::default(),
        end: byte_offset_to_position(text, text.len()),
    }
}

/// Format Structured Text: normalize keyword casing (trivia-preserving),
/// re-indent block structure, and trim trailing whitespace.
fn format_text(text: &str) -> String {
    let cased = case_keywords(text);
    reindent(&cased)
}

fn case_keywords(text: &str) -> String {
    let parse = parse_source(text);
    let mut out = String::with_capacity(text.len());
    for token in parse.tokens() {
        if token.kind == TokenKind::Keyword {
            out.push_str(&token.text.to_ascii_uppercase());
        } else {
            out.push_str(&token.text);
        }
    }
    out
}

fn reindent(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth: usize = 0;

    for line in text.lines() {
        let trimmed = line.trim();
        let first = trimmed
            .split(|c: char| c.is_whitespace())
            .next()
            .unwrap_or("")
            .to_ascii_uppercase();

        if trimmed.is_empty() {
            out.push('\n');
            continue;
        }

        if is_block_closer(&first) {
            depth = depth.saturating_sub(1);
        }

        for _ in 0..depth {
            out.push_str("    ");
        }
        out.push_str(trimmed);
        out.push('\n');

        if is_block_opener(&first, trimmed) {
            depth += 1;
        }
    }

    out
}

fn is_block_opener(first: &str, line: &str) -> bool {
    matches!(
        first,
        "PROGRAM"
            | "FUNCTION"
            | "FUNCTION_BLOCK"
            | "ACTION"
            | "VAR"
            | "VAR_INPUT"
            | "VAR_OUTPUT"
            | "VAR_IN_OUT"
            | "VAR_GLOBAL"
            | "VAR_TEMP"
            | "FOR"
            | "WHILE"
            | "CASE"
            | "REPEAT"
    ) || (first == "IF" && line.to_ascii_uppercase().trim_end().ends_with("THEN"))
}

fn is_block_closer(first: &str) -> bool {
    matches!(
        first,
        "END_PROGRAM"
            | "END_FUNCTION"
            | "END_FUNCTION_BLOCK"
            | "END_ACTION"
            | "END_VAR"
            | "END_IF"
            | "END_FOR"
            | "END_WHILE"
            | "END_CASE"
            | "END_REPEAT"
            | "UNTIL"
    )
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

fn workspace_symbols(documents: &[SourceDocument], query: &str) -> Vec<WorkspaceSymbol> {
    let files: Vec<SourceFile> = documents
        .iter()
        .map(|document| SourceFile::new(document.uri(), document.text()))
        .collect();
    let semantic = analyze_workspace(&files);
    let needle = query.to_ascii_lowercase();

    semantic
        .symbol_index
        .symbols()
        .iter()
        .filter(|symbol| symbol.container.is_none())
        .filter(|symbol| needle.is_empty() || symbol.name.to_ascii_lowercase().contains(&needle))
        .filter_map(|symbol| {
            let text = documents
                .iter()
                .find(|document| document.uri() == symbol.uri)
                .map(SourceDocument::text)?;
            Some(WorkspaceSymbol {
                name: symbol.name.clone(),
                kind: symbol_kind(symbol.kind),
                location: Location {
                    uri: symbol.uri.clone(),
                    range: text_range_to_range(text, symbol.range),
                },
                container_name: symbol.container.clone(),
            })
        })
        .collect()
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

fn completion_candidates(
    document: &SourceDocument,
    position: Position,
) -> Vec<CompletionCandidate> {
    // Member-access context (`inst.`) yields the instance's members only.
    if let Some(members) = member_access_candidates(document.text(), position) {
        return members;
    }
    global_candidates(document)
}

fn global_candidates(document: &SourceDocument) -> Vec<CompletionCandidate> {
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
    candidates.extend(
        STANDARD_FUNCTION_NAMES
            .iter()
            .map(|name| CompletionCandidate {
                label: (*name).to_owned(),
                detail: Some("standard function".to_owned()),
                kind: SymbolKind::Function,
            }),
    );
    candidates.extend(
        STANDARD_FUNCTION_BLOCKS
            .iter()
            .map(|(name, _)| CompletionCandidate {
                label: (*name).to_owned(),
                detail: Some("standard function block".to_owned()),
                kind: SymbolKind::FunctionBlock,
            }),
    );
    candidates.sort_by(|left, right| left.label.cmp(&right.label));
    candidates.dedup_by(|left, right| left.label.eq_ignore_ascii_case(&right.label));
    candidates
}

/// When the cursor sits on a member access (`base.` or `base.partial`), return
/// the members of `base`'s function-block type. Returns `None` when the cursor
/// is not in a member-access context (so the caller falls back to the global
/// list); returns `Some(vec)` — possibly empty — once a `base.` context is
/// detected, so the global list is suppressed after a dot.
fn member_access_candidates(text: &str, position: Position) -> Option<Vec<CompletionCandidate>> {
    let offset = position_to_byte_offset(text, position)?;
    let parse = parse_source(text);
    let tokens: Vec<&Token> = parse
        .tokens()
        .iter()
        .filter(|token| !token.is_trivia())
        .collect();
    let base = member_access_base(&tokens, offset)?;
    Some(fb_member_candidates(parse.units(), &base))
}

/// Identify the base identifier of a member access ending at `offset`, matching
/// either `base.` or `base.partial` immediately before the cursor.
fn member_access_base(tokens: &[&Token], offset: usize) -> Option<String> {
    let last = tokens
        .iter()
        .rposition(|token| token.range.start < offset)?;
    let dot = match tokens[last].kind {
        TokenKind::Operator if tokens[last].text == "." => last,
        TokenKind::Identifier => {
            let previous = last.checked_sub(1)?;
            if tokens[previous].kind == TokenKind::Operator && tokens[previous].text == "." {
                previous
            } else {
                return None;
            }
        }
        _ => return None,
    };

    let base = tokens.get(dot.checked_sub(1)?)?;
    (base.kind == TokenKind::Identifier).then(|| base.text.clone())
}

fn fb_member_candidates(units: &[Pou], base: &str) -> Vec<CompletionCandidate> {
    let Some(type_name) = variable_type_name(units, base) else {
        return Vec::new();
    };

    if let Some(unit) = units
        .iter()
        .find(|unit| unit.kind == PouKind::FunctionBlock && pou_named(unit, &type_name))
    {
        return unit
            .declaration_blocks
            .iter()
            .filter(|block| {
                matches!(
                    block.kind,
                    VarBlockKind::Input | VarBlockKind::Output | VarBlockKind::InOut
                )
            })
            .flat_map(|block| block.declarations.iter())
            .map(|declaration| CompletionCandidate {
                label: declaration.name.clone(),
                detail: Some(format!("member of {}", declaration.type_name)),
                kind: SymbolKind::Variable,
            })
            .collect();
    }

    standard_fb_members(&type_name)
        .map(|members| {
            members
                .iter()
                .map(|(name, type_name)| CompletionCandidate {
                    label: (*name).to_owned(),
                    detail: Some(format!("member of {type_name}")),
                    kind: SymbolKind::Variable,
                })
                .collect()
        })
        .unwrap_or_default()
}

fn standard_fb_members(name: &str) -> Option<&'static [(&'static str, &'static str)]> {
    let upper = name.to_ascii_uppercase();
    STANDARD_FUNCTION_BLOCKS
        .iter()
        .find(|(candidate, _)| *candidate == upper)
        .map(|(_, members)| *members)
}

/// MVP standard functions surfaced in completion. Mirrors
/// `plc_runtime::stdlib::STANDARD_FUNCTIONS` (kept self-contained so compiler-core
/// stays free of a runtime dependency) — keep the two sets in sync.
const STANDARD_FUNCTION_NAMES: &[&str] = &[
    "ABS",
    "SQRT",
    "MIN",
    "MAX",
    "LIMIT",
    "SEL",
    "LEN",
    "CONCAT",
    "INT_TO_REAL",
    "REAL_TO_INT",
    "BOOL_TO_INT",
    "INT_TO_STRING",
];

/// MVP standard function blocks and their public members (name, type). Mirrors
/// the `plc_runtime` timer/counter/edge surface — keep in sync with that crate.
const STANDARD_FUNCTION_BLOCKS: &[(&str, &[(&str, &str)])] = &[
    (
        "TON",
        &[
            ("IN", "BOOL"),
            ("PT", "TIME"),
            ("Q", "BOOL"),
            ("ET", "TIME"),
        ],
    ),
    (
        "TOF",
        &[
            ("IN", "BOOL"),
            ("PT", "TIME"),
            ("Q", "BOOL"),
            ("ET", "TIME"),
        ],
    ),
    (
        "TP",
        &[
            ("IN", "BOOL"),
            ("PT", "TIME"),
            ("Q", "BOOL"),
            ("ET", "TIME"),
        ],
    ),
    (
        "CTU",
        &[
            ("CU", "BOOL"),
            ("R", "BOOL"),
            ("PV", "INT"),
            ("Q", "BOOL"),
            ("CV", "INT"),
        ],
    ),
    (
        "CTD",
        &[
            ("CD", "BOOL"),
            ("LD", "BOOL"),
            ("PV", "INT"),
            ("Q", "BOOL"),
            ("CV", "INT"),
        ],
    ),
    (
        "CTUD",
        &[
            ("CU", "BOOL"),
            ("CD", "BOOL"),
            ("R", "BOOL"),
            ("LD", "BOOL"),
            ("PV", "INT"),
            ("QU", "BOOL"),
            ("QD", "BOOL"),
            ("CV", "INT"),
        ],
    ),
    ("R_TRIG", &[("CLK", "BOOL"), ("Q", "BOOL")]),
    ("F_TRIG", &[("CLK", "BOOL"), ("Q", "BOOL")]),
];

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

/// Resolved call signature: the name to display plus its ordered parameters.
struct ResolvedSignature {
    display_name: String,
    parameters: Vec<ParameterInfo>,
}

fn signature_help_at_position(
    document: &SourceDocument,
    position: Position,
) -> Option<SignatureInfo> {
    let text = document.text();
    let offset = position_to_byte_offset(text, position)?;
    let (callee, active_parameter) = enclosing_call(text, offset)?;
    let resolved = resolve_callee(text, &callee)?;

    let active_parameter = if resolved.parameters.is_empty() {
        None
    } else {
        Some(active_parameter.min(resolved.parameters.len() as u32 - 1))
    };

    Some(SignatureInfo {
        label: signature_label(&resolved.display_name, &resolved.parameters),
        parameters: resolved.parameters,
        active_parameter,
    })
}

/// Locate the call enclosing `offset`, returning the callee name and the
/// zero-based index of the argument the cursor sits in. Walks the token stream
/// keeping a stack of open parentheses; only parentheses immediately preceded by
/// an identifier are treated as calls (others are grouping parentheses).
fn enclosing_call(text: &str, offset: usize) -> Option<(String, u32)> {
    struct CallFrame {
        name: Option<String>,
        active_parameter: u32,
    }

    let parse = parse_source(text);
    let mut stack: Vec<CallFrame> = Vec::new();
    let mut previous_identifier: Option<String> = None;

    for token in parse
        .tokens()
        .iter()
        .filter(|token| token.range.start < offset && !token.is_trivia())
    {
        if token.kind == TokenKind::Operator {
            match token.text.as_str() {
                "(" => stack.push(CallFrame {
                    name: previous_identifier.clone(),
                    active_parameter: 0,
                }),
                ")" => {
                    stack.pop();
                }
                "," => {
                    if let Some(frame) = stack.last_mut() {
                        frame.active_parameter += 1;
                    }
                }
                _ => {}
            }
        }

        previous_identifier = (token.kind == TokenKind::Identifier).then(|| token.text.clone());
    }

    stack.iter().rev().find_map(|frame| {
        frame
            .name
            .clone()
            .map(|name| (name, frame.active_parameter))
    })
}

fn resolve_callee(text: &str, name: &str) -> Option<ResolvedSignature> {
    standard_signature(name).or_else(|| user_signature(text, name))
}

/// Declarative signatures for the MVP standard functions. Mirrors
/// `plc_runtime::stdlib::STANDARD_FUNCTIONS` (kept self-contained so compiler-core
/// stays free of a runtime dependency) — keep the two sets in sync.
fn standard_signature(name: &str) -> Option<ResolvedSignature> {
    let upper = name.to_ascii_uppercase();
    let parameters: &[(&str, &str)] = match upper.as_str() {
        "ABS" => &[("IN", "ANY_NUM")],
        "SQRT" => &[("IN", "ANY_NUM")],
        "MIN" => &[("IN1", "ANY_NUM"), ("IN2", "ANY_NUM")],
        "MAX" => &[("IN1", "ANY_NUM"), ("IN2", "ANY_NUM")],
        "LIMIT" => &[("MN", "ANY_NUM"), ("IN", "ANY_NUM"), ("MX", "ANY_NUM")],
        "SEL" => &[("G", "BOOL"), ("IN0", "ANY"), ("IN1", "ANY")],
        "LEN" => &[("IN", "STRING")],
        "CONCAT" => &[("IN1", "STRING"), ("IN2", "STRING")],
        "INT_TO_REAL" => &[("IN", "INT")],
        "REAL_TO_INT" => &[("IN", "REAL")],
        "BOOL_TO_INT" => &[("IN", "BOOL")],
        "INT_TO_STRING" => &[("IN", "INT")],
        _ => return None,
    };

    Some(ResolvedSignature {
        parameters: parameters
            .iter()
            .map(|(parameter_name, type_name)| ParameterInfo {
                label: format!("{parameter_name} : {type_name}"),
            })
            .collect(),
        display_name: upper,
    })
}

/// Resolve a user-declared `FUNCTION`/`FUNCTION_BLOCK` call. Matches the callee
/// against a POU name directly, then falls back to a function-block instance
/// whose declared type names a function block.
fn user_signature(text: &str, name: &str) -> Option<ResolvedSignature> {
    let parse = parse_source(text);
    let units = parse.units();

    if let Some(unit) = units.iter().find(|unit| {
        matches!(unit.kind, PouKind::Function | PouKind::FunctionBlock) && pou_named(unit, name)
    }) {
        return Some(ResolvedSignature {
            display_name: unit.name.clone().unwrap_or_else(|| name.to_owned()),
            parameters: input_parameters(unit),
        });
    }

    let type_name = variable_type_name(units, name)?;
    let function_block = units
        .iter()
        .find(|unit| unit.kind == PouKind::FunctionBlock && pou_named(unit, &type_name))?;
    Some(ResolvedSignature {
        display_name: function_block.name.clone().unwrap_or(type_name),
        parameters: input_parameters(function_block),
    })
}

fn pou_named(unit: &Pou, name: &str) -> bool {
    unit.name
        .as_deref()
        .is_some_and(|unit_name| unit_name.eq_ignore_ascii_case(name))
}

fn input_parameters(unit: &Pou) -> Vec<ParameterInfo> {
    unit.declaration_blocks
        .iter()
        .filter(|block| matches!(block.kind, VarBlockKind::Input | VarBlockKind::InOut))
        .flat_map(|block| block.declarations.iter())
        .map(|declaration| ParameterInfo {
            label: format!("{} : {}", declaration.name, declaration.type_name),
        })
        .collect()
}

fn variable_type_name(units: &[Pou], name: &str) -> Option<String> {
    units
        .iter()
        .flat_map(|unit| unit.declaration_blocks.iter())
        .flat_map(|block| block.declarations.iter())
        .find(|declaration| declaration.name.eq_ignore_ascii_case(name))
        .map(|declaration| declaration.type_name.clone())
}

fn signature_label(name: &str, parameters: &[ParameterInfo]) -> String {
    let joined = parameters
        .iter()
        .map(|parameter| parameter.label.as_str())
        .collect::<Vec<_>>()
        .join("; ");
    format!("{name}({joined})")
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

fn identifier_at_position(text: &str, position: Position) -> Option<(String, TextRange)> {
    let offset = position_to_byte_offset(text, position)?;
    let parse = parse_source(text);
    let token = parse
        .tokens()
        .iter()
        .find(|token| token.range.start <= offset && offset <= token.range.end)?;
    if token.kind != TokenKind::Identifier {
        return None;
    }
    Some((token.text.clone(), token.range))
}

fn definition_at_position(document: &SourceDocument, position: Position) -> Option<Location> {
    let (name, _) = identifier_at_position(document.text(), position)?;
    let semantic = analyze_file(document.uri(), document.text());
    let symbol = semantic
        .symbol_index
        .symbols()
        .iter()
        .find(|symbol| symbol.name.eq_ignore_ascii_case(&name))?;
    Some(Location {
        uri: document.uri().to_owned(),
        range: text_range_to_range(document.text(), symbol.range),
    })
}

fn references_at_position(
    document: &SourceDocument,
    position: Position,
    include_declaration: bool,
) -> Vec<Location> {
    let Some((name, _)) = identifier_at_position(document.text(), position) else {
        return Vec::new();
    };

    let parse = parse_source(document.text());
    let mut locations: Vec<Location> = parse
        .tokens()
        .iter()
        .filter(|token| {
            token.kind == TokenKind::Identifier && token.text.eq_ignore_ascii_case(&name)
        })
        .map(|token| Location {
            uri: document.uri().to_owned(),
            range: text_range_to_range(document.text(), token.range),
        })
        .collect();

    if !include_declaration {
        let semantic = analyze_file(document.uri(), document.text());
        if let Some(symbol) = semantic
            .symbol_index
            .symbols()
            .iter()
            .find(|symbol| symbol.name.eq_ignore_ascii_case(&name))
        {
            let declaration = text_range_to_range(document.text(), symbol.range);
            locations.retain(|location| location.range != declaration);
        }
    }

    locations
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
