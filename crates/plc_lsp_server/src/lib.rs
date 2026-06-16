//! LSP server implementation for PLC VS Code.

use plc_compiler_core::{
    CodeAction as CoreCodeAction, CompilerCore, CompletionCandidate as CoreCompletionCandidate,
    DiagnosticSeverity as CoreSeverity, DocumentSymbol as CoreDocumentSymbol,
    HoverInfo as CoreHoverInfo, LanguageService, Location as CoreLocation,
    Position as CorePosition, Range as CoreRange, SemanticTokenKind as CoreSemanticTokenKind,
    SignatureInfo as CoreSignatureInfo, SourceDocument, SymbolKind as CoreSymbolKind,
    TextEdit as CoreTextEdit, WorkspaceSymbol as CoreWorkspaceSymbol,
};
use plc_lang::LanguageRegistry;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams,
    CodeActionProviderCapability, CodeActionResponse, CompletionItem, CompletionItemKind,
    CompletionOptions, CompletionParams, CompletionResponse, Diagnostic, DiagnosticSeverity,
    DidChangeTextDocumentParams, DidOpenTextDocumentParams, DocumentFormattingParams,
    DocumentRangeFormattingParams, DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse,
    GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverContents, HoverParams,
    InitializeParams, InitializeResult, Location, MarkupContent, MarkupKind, MessageType, OneOf,
    ParameterInformation, ParameterLabel, Position, Range, ReferenceParams, SemanticToken,
    SemanticTokenType, SemanticTokens, SemanticTokensFullOptions, SemanticTokensLegend,
    SemanticTokensOptions, SemanticTokensParams, SemanticTokensResult,
    SemanticTokensServerCapabilities, ServerCapabilities, SignatureHelp, SignatureHelpOptions,
    SignatureHelpParams, SignatureInformation, SymbolInformation, SymbolKind,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit, Url, WorkspaceEdit,
    WorkspaceSymbolParams,
};
use tower_lsp::{Client, LanguageServer};

/// Convert compiler-core analysis into LSP diagnostics.
pub fn diagnostics_for_text(uri: &str, version: i32, text: &str) -> Vec<Diagnostic> {
    diagnostics_with(&CompilerCore, uri, version, text)
}

/// Backend-parameterized core of [`diagnostics_for_text`]; works with any `LanguageService`.
fn diagnostics_with(
    core: &dyn LanguageService,
    uri: &str,
    version: i32,
    text: &str,
) -> Vec<Diagnostic> {
    let document = SourceDocument::new(uri, version, text);
    core.analyze(&document)
        .diagnostics()
        .iter()
        .map(|diagnostic| Diagnostic {
            range: lsp_range(diagnostic.range),
            severity: Some(match diagnostic.severity {
                CoreSeverity::Error => DiagnosticSeverity::ERROR,
                CoreSeverity::Warning => DiagnosticSeverity::WARNING,
                CoreSeverity::Information => DiagnosticSeverity::INFORMATION,
                CoreSeverity::Hint => DiagnosticSeverity::HINT,
            }),
            code: Some(tower_lsp::lsp_types::NumberOrString::String(
                diagnostic.code.to_owned(),
            )),
            code_description: None,
            source: Some("plc-vscode".to_owned()),
            message: diagnostic.message.clone(),
            related_information: None,
            tags: None,
            data: None,
        })
        .collect()
}

/// Convert compiler-core document symbols into LSP nested document symbols.
pub fn document_symbols_for_text(uri: &str, version: i32, text: &str) -> Vec<DocumentSymbol> {
    document_symbols_with(&CompilerCore, uri, version, text)
}

/// Backend-parameterized core of [`document_symbols_for_text`]; works with any `LanguageService`.
fn document_symbols_with(
    core: &dyn LanguageService,
    uri: &str,
    version: i32,
    text: &str,
) -> Vec<DocumentSymbol> {
    let document = SourceDocument::new(uri, version, text);
    core.document_symbols(&document)
        .symbols()
        .iter()
        .map(lsp_document_symbol)
        .collect()
}

/// Query top-level workspace symbols across `(uri, text)` documents and map them
/// to LSP `SymbolInformation`. Symbols whose URI fails to parse are skipped.
pub fn workspace_symbols_for_documents(
    documents: &[(String, String)],
    query: &str,
) -> Vec<SymbolInformation> {
    workspace_symbols_with(&CompilerCore, documents, query)
}

/// Backend-parameterized core of [`workspace_symbols_for_documents`]; works with any `LanguageService`.
fn workspace_symbols_with(
    core: &dyn LanguageService,
    documents: &[(String, String)],
    query: &str,
) -> Vec<SymbolInformation> {
    let sources: Vec<SourceDocument> = documents
        .iter()
        .map(|(uri, text)| SourceDocument::new(uri.clone(), 0, text.clone()))
        .collect();
    core.workspace_symbols(&sources, query)
        .into_iter()
        .filter_map(lsp_symbol_information)
        .collect()
}

#[allow(deprecated)]
fn lsp_symbol_information(symbol: CoreWorkspaceSymbol) -> Option<SymbolInformation> {
    Some(SymbolInformation {
        name: symbol.name,
        kind: lsp_symbol_kind(symbol.kind),
        tags: None,
        deprecated: None,
        location: Location {
            uri: Url::parse(&symbol.location.uri).ok()?,
            range: lsp_range(symbol.location.range),
        },
        container_name: symbol.container_name,
    })
}

/// Stable semantic tokens legend. The order of `token_types` defines the
/// numeric `token_type` indices used in encoded tokens — keep it in sync with
/// `semantic_token_type_index`.
pub fn semantic_tokens_legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![
            SemanticTokenType::KEYWORD,
            SemanticTokenType::TYPE,
            SemanticTokenType::VARIABLE,
            SemanticTokenType::FUNCTION,
            SemanticTokenType::CLASS,
            SemanticTokenType::NUMBER,
            SemanticTokenType::STRING,
            SemanticTokenType::COMMENT,
            SemanticTokenType::OPERATOR,
        ],
        token_modifiers: Vec::new(),
    }
}

fn semantic_token_type_index(kind: CoreSemanticTokenKind) -> u32 {
    match kind {
        CoreSemanticTokenKind::Keyword => 0,
        CoreSemanticTokenKind::Type => 1,
        CoreSemanticTokenKind::Variable => 2,
        CoreSemanticTokenKind::Function => 3,
        CoreSemanticTokenKind::FunctionBlock => 4,
        CoreSemanticTokenKind::Number => 5,
        CoreSemanticTokenKind::String => 6,
        CoreSemanticTokenKind::Comment => 7,
        CoreSemanticTokenKind::Operator => 8,
    }
}

/// Build delta-encoded LSP semantic tokens for a document. Compiler-core yields
/// single-line tokens in source order, so encoding is a straight delta pass.
pub fn semantic_tokens_for_text(uri: &str, version: i32, text: &str) -> SemanticTokens {
    semantic_tokens_with(&CompilerCore, uri, version, text)
}

/// Backend-parameterized core of [`semantic_tokens_for_text`]; works with any `LanguageService`.
fn semantic_tokens_with(
    core: &dyn LanguageService,
    uri: &str,
    version: i32,
    text: &str,
) -> SemanticTokens {
    let document = SourceDocument::new(uri, version, text);

    let mut data = Vec::new();
    let mut last_line = 0u32;
    let mut last_start = 0u32;

    for token in core.semantic_tokens(&document) {
        let line = token.range.start.line;
        let start = token.range.start.character;
        let length = token.range.end.character.saturating_sub(start);
        let delta_line = line - last_line;
        let delta_start = if delta_line == 0 {
            start - last_start
        } else {
            start
        };

        data.push(SemanticToken {
            delta_line,
            delta_start,
            length,
            token_type: semantic_token_type_index(token.kind),
            token_modifiers_bitset: 0,
        });

        last_line = line;
        last_start = start;
    }

    SemanticTokens {
        result_id: None,
        data,
    }
}

#[allow(deprecated)]
fn lsp_document_symbol(symbol: &CoreDocumentSymbol) -> DocumentSymbol {
    DocumentSymbol {
        name: symbol.name.clone(),
        detail: symbol.detail.clone(),
        kind: lsp_symbol_kind(symbol.kind),
        tags: None,
        deprecated: None,
        range: lsp_range(symbol.range),
        selection_range: lsp_range(symbol.selection_range),
        children: if symbol.children.is_empty() {
            None
        } else {
            Some(symbol.children.iter().map(lsp_document_symbol).collect())
        },
    }
}

fn lsp_symbol_kind(kind: CoreSymbolKind) -> SymbolKind {
    match kind {
        CoreSymbolKind::Program => SymbolKind::MODULE,
        CoreSymbolKind::Function => SymbolKind::FUNCTION,
        CoreSymbolKind::FunctionBlock => SymbolKind::CLASS,
        CoreSymbolKind::Action => SymbolKind::METHOD,
        CoreSymbolKind::Variable => SymbolKind::VARIABLE,
        CoreSymbolKind::Type => SymbolKind::STRUCT,
        CoreSymbolKind::Keyword => SymbolKind::KEY,
    }
}

fn lsp_range(range: CoreRange) -> Range {
    Range {
        start: Position {
            line: range.start.line,
            character: range.start.character,
        },
        end: Position {
            line: range.end.line,
            character: range.end.character,
        },
    }
}

/// Convert compiler-core completion candidates into LSP completion items.
pub fn completion_items_for_text(
    uri: &str,
    version: i32,
    text: &str,
    position: Position,
) -> Vec<CompletionItem> {
    completion_items_with(&CompilerCore, uri, version, text, position)
}

/// Backend-parameterized core of [`completion_items_for_text`]; works with any `LanguageService`.
fn completion_items_with(
    core: &dyn LanguageService,
    uri: &str,
    version: i32,
    text: &str,
    position: Position,
) -> Vec<CompletionItem> {
    let document = SourceDocument::new(uri, version, text);
    core.completions(
        &document,
        CorePosition {
            line: position.line,
            character: position.character,
        },
    )
    .iter()
    .map(lsp_completion_item)
    .collect()
}

/// Convert compiler-core hover payloads into LSP hover responses.
pub fn hover_for_text(uri: &str, version: i32, text: &str, position: Position) -> Option<Hover> {
    hover_with(&CompilerCore, uri, version, text, position)
}

/// Backend-parameterized core of [`hover_for_text`]; works with any `LanguageService`.
fn hover_with(
    core: &dyn LanguageService,
    uri: &str,
    version: i32,
    text: &str,
    position: Position,
) -> Option<Hover> {
    let document = SourceDocument::new(uri, version, text);
    core.hover(
        &document,
        CorePosition {
            line: position.line,
            character: position.character,
        },
    )
    .map(lsp_hover)
}

/// Convert compiler-core call signature data into LSP signature help.
pub fn signature_help_for_text(
    uri: &str,
    version: i32,
    text: &str,
    position: Position,
) -> Option<SignatureHelp> {
    signature_help_with(&CompilerCore, uri, version, text, position)
}

/// Backend-parameterized core of [`signature_help_for_text`]; works with any `LanguageService`.
fn signature_help_with(
    core: &dyn LanguageService,
    uri: &str,
    version: i32,
    text: &str,
    position: Position,
) -> Option<SignatureHelp> {
    let document = SourceDocument::new(uri, version, text);
    core.signature_help(
        &document,
        CorePosition {
            line: position.line,
            character: position.character,
        },
    )
    .map(lsp_signature_help)
}

fn lsp_signature_help(signature: CoreSignatureInfo) -> SignatureHelp {
    let active_parameter = signature.active_parameter;
    let parameters = signature
        .parameters
        .into_iter()
        .map(|parameter| ParameterInformation {
            label: ParameterLabel::Simple(parameter.label),
            documentation: None,
        })
        .collect();

    SignatureHelp {
        signatures: vec![SignatureInformation {
            label: signature.label,
            documentation: None,
            parameters: Some(parameters),
            active_parameter,
        }],
        active_signature: Some(0),
        active_parameter,
    }
}

fn lsp_completion_item(candidate: &CoreCompletionCandidate) -> CompletionItem {
    CompletionItem {
        label: candidate.label.clone(),
        kind: Some(lsp_completion_kind(candidate.kind)),
        detail: candidate.detail.clone(),
        ..CompletionItem::default()
    }
}

fn lsp_completion_kind(kind: CoreSymbolKind) -> CompletionItemKind {
    match kind {
        CoreSymbolKind::Program => CompletionItemKind::MODULE,
        CoreSymbolKind::Function => CompletionItemKind::FUNCTION,
        CoreSymbolKind::FunctionBlock => CompletionItemKind::CLASS,
        CoreSymbolKind::Action => CompletionItemKind::METHOD,
        CoreSymbolKind::Variable => CompletionItemKind::VARIABLE,
        CoreSymbolKind::Type => CompletionItemKind::STRUCT,
        CoreSymbolKind::Keyword => CompletionItemKind::KEYWORD,
    }
}

fn lsp_hover(hover: CoreHoverInfo) -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: hover.contents,
        }),
        range: Some(lsp_range(hover.range)),
    }
}

/// Resolve a go-to-definition location through compiler-core.
pub fn definition_for_text(
    uri: &str,
    version: i32,
    text: &str,
    position: Position,
) -> Option<Location> {
    definition_with(&CompilerCore, uri, version, text, position)
}

/// Backend-parameterized core of [`definition_for_text`]; works with any `LanguageService`.
fn definition_with(
    core: &dyn LanguageService,
    uri: &str,
    version: i32,
    text: &str,
    position: Position,
) -> Option<Location> {
    let document = SourceDocument::new(uri, version, text);
    core.definition(
        &document,
        CorePosition {
            line: position.line,
            character: position.character,
        },
    )
    .and_then(lsp_location)
}

/// Resolve find-references locations through compiler-core.
pub fn references_for_text(
    uri: &str,
    version: i32,
    text: &str,
    position: Position,
    include_declaration: bool,
) -> Vec<Location> {
    references_with(
        &CompilerCore,
        uri,
        version,
        text,
        position,
        include_declaration,
    )
}

/// Backend-parameterized core of [`references_for_text`]; works with any `LanguageService`.
fn references_with(
    core: &dyn LanguageService,
    uri: &str,
    version: i32,
    text: &str,
    position: Position,
    include_declaration: bool,
) -> Vec<Location> {
    let document = SourceDocument::new(uri, version, text);
    core.references(
        &document,
        CorePosition {
            line: position.line,
            character: position.character,
        },
        include_declaration,
    )
    .into_iter()
    .filter_map(lsp_location)
    .collect()
}

fn lsp_location(location: CoreLocation) -> Option<Location> {
    let uri = Url::parse(&location.uri).ok()?;
    Some(Location {
        uri,
        range: lsp_range(location.range),
    })
}

/// Produce whole-document formatting edits through compiler-core.
pub fn formatting_edits_for_text(uri: &str, version: i32, text: &str) -> Vec<TextEdit> {
    formatting_edits_with(&CompilerCore, uri, version, text)
}

/// Backend-parameterized core of [`formatting_edits_for_text`]; works with any `LanguageService`.
fn formatting_edits_with(
    core: &dyn LanguageService,
    uri: &str,
    version: i32,
    text: &str,
) -> Vec<TextEdit> {
    let document = SourceDocument::new(uri, version, text);
    core.formatting(&document)
        .into_iter()
        .map(lsp_text_edit)
        .collect()
}

/// Produce range formatting edits through compiler-core.
pub fn range_formatting_edits_for_text(
    uri: &str,
    version: i32,
    text: &str,
    range: Range,
) -> Vec<TextEdit> {
    range_formatting_edits_with(&CompilerCore, uri, version, text, range)
}

/// Backend-parameterized core of [`range_formatting_edits_for_text`]; works with any `LanguageService`.
fn range_formatting_edits_with(
    core: &dyn LanguageService,
    uri: &str,
    version: i32,
    text: &str,
    range: Range,
) -> Vec<TextEdit> {
    let document = SourceDocument::new(uri, version, text);
    core.formatting_range(
        &document,
        CoreRange {
            start: CorePosition {
                line: range.start.line,
                character: range.start.character,
            },
            end: CorePosition {
                line: range.end.line,
                character: range.end.character,
            },
        },
    )
    .into_iter()
    .map(lsp_text_edit)
    .collect()
}

fn lsp_text_edit(edit: CoreTextEdit) -> TextEdit {
    TextEdit {
        range: lsp_range(edit.range),
        new_text: edit.new_text,
    }
}

fn lsp_code_action(uri: &Url, action: CoreCodeAction) -> CodeActionOrCommand {
    let edits: Vec<TextEdit> = action.edits.into_iter().map(lsp_text_edit).collect();
    let mut changes = HashMap::new();
    changes.insert(uri.clone(), edits);
    CodeActionOrCommand::CodeAction(CodeAction {
        title: action.title,
        kind: Some(CodeActionKind::QUICKFIX),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..WorkspaceEdit::default()
        }),
        ..CodeAction::default()
    })
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct DocumentSnapshot {
    version: i32,
    text: String,
}

/// Language server backend.
pub struct PlcLanguageServer {
    client: Client,
    documents: Arc<RwLock<HashMap<Url, DocumentSnapshot>>>,
    /// Pluggable analysis backend. Defaults to `CompilerCore`; swap via
    /// [`PlcLanguageServer::with_service`] to bring your own.
    service: Arc<dyn LanguageService + Send + Sync>,
    /// Optional language registry. When set, the analyzer is chosen per document
    /// by file extension (multi-language). `None` => always use `service`.
    registry: Option<Arc<LanguageRegistry>>,
}

impl PlcLanguageServer {
    /// Construct a server backed by the default `CompilerCore` analyzer.
    pub fn new(client: Client) -> Self {
        Self::with_service(client, Arc::new(CompilerCore))
    }

    /// Construct a server backed by any [`LanguageService`] — bring your own
    /// analyzer/compiler frontend behind the provided tower-lsp host.
    pub fn with_service(client: Client, service: Arc<dyn LanguageService + Send + Sync>) -> Self {
        Self {
            client,
            documents: Arc::new(RwLock::new(HashMap::new())),
            service,
            registry: None,
        }
    }

    /// Construct a language-aware server: the analyzer is selected per document
    /// by file extension from `registry` (e.g. `.st` -> CompilerCore, `.il` ->
    /// IL diagnostics), falling back to `CompilerCore` for unknown languages.
    pub fn with_registry(client: Client, registry: Arc<LanguageRegistry>) -> Self {
        Self {
            client,
            documents: Arc::new(RwLock::new(HashMap::new())),
            service: Arc::new(CompilerCore),
            registry: Some(registry),
        }
    }

    /// Resolve the analysis backend for a document URI: the per-language service
    /// from the registry if one is set, otherwise the fixed `service`.
    fn service_for(&self, uri: &str) -> Arc<dyn LanguageService + Send + Sync> {
        self.registry
            .as_ref()
            .and_then(|registry| registry.language_service_for_uri(uri))
            .unwrap_or_else(|| Arc::clone(&self.service))
    }

    async fn publish_for(&self, uri: Url, version: i32, text: &str) {
        let diagnostics = diagnostics_with(
            self.service_for(uri.as_str()).as_ref(),
            uri.as_str(),
            version,
            text,
        );
        self.client
            .publish_diagnostics(
                uri,
                diagnostics,
                if version >= 0 { Some(version) } else { None },
            )
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for PlcLanguageServer {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: server_capabilities(),
            server_info: Some(tower_lsp::lsp_types::ServerInfo {
                name: "PLC VS Code Language Server".to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
        })
    }

    async fn initialized(&self, _: tower_lsp::lsp_types::InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "PLC VS Code language server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let doc = params.text_document;
        let version = doc.version;
        let text = doc.text;
        let uri = doc.uri;
        self.documents.write().await.insert(
            uri.clone(),
            DocumentSnapshot {
                version,
                text: text.clone(),
            },
        );
        self.publish_for(uri, version, &text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        let text = if let Some(change) = params.content_changes.into_iter().next() {
            change.text
        } else {
            self.documents
                .read()
                .await
                .get(&uri)
                .map(|snapshot| snapshot.text.clone())
                .unwrap_or_default()
        };

        self.documents.write().await.insert(
            uri.clone(),
            DocumentSnapshot {
                version,
                text: text.clone(),
            },
        );
        self.publish_for(uri, version, &text).await;
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let documents = self.documents.read().await;
        Ok(documents.get(&uri).map(|snapshot| {
            DocumentSymbolResponse::Nested(document_symbols_with(
                self.service_for(uri.as_str()).as_ref(),
                uri.as_str(),
                snapshot.version,
                &snapshot.text,
            ))
        }))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let documents = self.documents.read().await;
        Ok(documents.get(&uri).map(|snapshot| {
            SemanticTokensResult::Tokens(semantic_tokens_with(
                self.service_for(uri.as_str()).as_ref(),
                uri.as_str(),
                snapshot.version,
                &snapshot.text,
            ))
        }))
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let documents: Vec<(String, String)> = self
            .documents
            .read()
            .await
            .iter()
            .map(|(uri, snapshot)| (uri.to_string(), snapshot.text.clone()))
            .collect();
        Ok(Some(workspace_symbols_with(
            self.service.as_ref(),
            &documents,
            &params.query,
        )))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let documents = self.documents.read().await;
        Ok(documents.get(&uri).map(|snapshot| {
            CompletionResponse::Array(completion_items_with(
                self.service_for(uri.as_str()).as_ref(),
                uri.as_str(),
                snapshot.version,
                &snapshot.text,
                position,
            ))
        }))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let documents = self.documents.read().await;
        Ok(documents.get(&uri).and_then(|snapshot| {
            hover_with(
                self.service_for(uri.as_str()).as_ref(),
                uri.as_str(),
                snapshot.version,
                &snapshot.text,
                position,
            )
        }))
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let documents = self.documents.read().await;
        Ok(documents.get(&uri).and_then(|snapshot| {
            signature_help_with(
                self.service_for(uri.as_str()).as_ref(),
                uri.as_str(),
                snapshot.version,
                &snapshot.text,
                position,
            )
        }))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let documents = self.documents.read().await;
        Ok(documents.get(&uri).and_then(|snapshot| {
            definition_with(
                self.service_for(uri.as_str()).as_ref(),
                uri.as_str(),
                snapshot.version,
                &snapshot.text,
                position,
            )
            .map(GotoDefinitionResponse::Scalar)
        }))
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let include_declaration = params.context.include_declaration;
        let documents = self.documents.read().await;
        Ok(documents.get(&uri).map(|snapshot| {
            references_with(
                self.service_for(uri.as_str()).as_ref(),
                uri.as_str(),
                snapshot.version,
                &snapshot.text,
                position,
                include_declaration,
            )
        }))
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let documents = self.documents.read().await;
        Ok(documents.get(&uri).map(|snapshot| {
            formatting_edits_with(
                self.service_for(uri.as_str()).as_ref(),
                uri.as_str(),
                snapshot.version,
                &snapshot.text,
            )
        }))
    }

    async fn range_formatting(
        &self,
        params: DocumentRangeFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let range = params.range;
        let documents = self.documents.read().await;
        Ok(documents.get(&uri).map(|snapshot| {
            range_formatting_edits_with(
                self.service_for(uri.as_str()).as_ref(),
                uri.as_str(),
                snapshot.version,
                &snapshot.text,
                range,
            )
        }))
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = params.text_document.uri;
        let documents = self.documents.read().await;
        Ok(documents.get(&uri).map(|snapshot| {
            let core = self.service_for(uri.as_str());
            let document = SourceDocument::new(uri.as_str(), snapshot.version, &snapshot.text);
            core.code_actions(&document)
                .into_iter()
                .map(|action| lsp_code_action(&uri, action))
                .collect::<CodeActionResponse>()
        }))
    }
}

/// Server capabilities helper used by tests and by `initialize`.
pub fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        document_symbol_provider: Some(OneOf::Left(true)),
        workspace_symbol_provider: Some(OneOf::Left(true)),
        semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(
            SemanticTokensOptions {
                legend: semantic_tokens_legend(),
                range: Some(false),
                full: Some(SemanticTokensFullOptions::Bool(true)),
                ..SemanticTokensOptions::default()
            },
        )),
        completion_provider: Some(CompletionOptions::default()),
        signature_help_provider: Some(SignatureHelpOptions::default()),
        hover_provider: Some(tower_lsp::lsp_types::HoverProviderCapability::Simple(true)),
        definition_provider: Some(OneOf::Left(true)),
        references_provider: Some(OneOf::Left(true)),
        document_formatting_provider: Some(OneOf::Left(true)),
        document_range_formatting_provider: Some(OneOf::Left(true)),
        code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
        ..ServerCapabilities::default()
    }
}
