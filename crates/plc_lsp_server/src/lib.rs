//! LSP server implementation for PLC VS Code.

use plc_compiler_core::{
    CodeAction as CoreCodeAction, CompilerCore, CompletionCandidate as CoreCompletionCandidate,
    DiagnosticSeverity as CoreSeverity, DocumentSymbol as CoreDocumentSymbol,
    HoverInfo as CoreHoverInfo, Location as CoreLocation, Position as CorePosition,
    Range as CoreRange, SourceDocument, SymbolKind as CoreSymbolKind, TextEdit as CoreTextEdit,
};
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
    Position, Range, ReferenceParams, ServerCapabilities, SymbolKind, TextDocumentSyncCapability,
    TextDocumentSyncKind, TextEdit, Url, WorkspaceEdit,
};
use tower_lsp::{Client, LanguageServer};

/// Convert compiler-core analysis into LSP diagnostics.
pub fn diagnostics_for_text(uri: &str, version: i32, text: &str) -> Vec<Diagnostic> {
    let core = CompilerCore;
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
    let core = CompilerCore;
    let document = SourceDocument::new(uri, version, text);
    core.document_symbols(&document)
        .symbols()
        .iter()
        .map(lsp_document_symbol)
        .collect()
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
pub fn completion_items_for_text(uri: &str, version: i32, text: &str) -> Vec<CompletionItem> {
    let core = CompilerCore;
    let document = SourceDocument::new(uri, version, text);
    core.completions(&document)
        .iter()
        .map(lsp_completion_item)
        .collect()
}

/// Convert compiler-core hover payloads into LSP hover responses.
pub fn hover_for_text(uri: &str, version: i32, text: &str, position: Position) -> Option<Hover> {
    let core = CompilerCore;
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
    let core = CompilerCore;
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
    let core = CompilerCore;
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
    let core = CompilerCore;
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
    let core = CompilerCore;
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
}

impl PlcLanguageServer {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn publish_for(&self, uri: Url, version: i32, text: &str) {
        let diagnostics = diagnostics_for_text(uri.as_str(), version, text);
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
            DocumentSymbolResponse::Nested(document_symbols_for_text(
                uri.as_str(),
                snapshot.version,
                &snapshot.text,
            ))
        }))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let documents = self.documents.read().await;
        Ok(documents.get(&uri).map(|snapshot| {
            CompletionResponse::Array(completion_items_for_text(
                uri.as_str(),
                snapshot.version,
                &snapshot.text,
            ))
        }))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let documents = self.documents.read().await;
        Ok(documents.get(&uri).and_then(|snapshot| {
            hover_for_text(uri.as_str(), snapshot.version, &snapshot.text, position)
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
            definition_for_text(uri.as_str(), snapshot.version, &snapshot.text, position)
                .map(GotoDefinitionResponse::Scalar)
        }))
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let include_declaration = params.context.include_declaration;
        let documents = self.documents.read().await;
        Ok(documents.get(&uri).map(|snapshot| {
            references_for_text(
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
            formatting_edits_for_text(uri.as_str(), snapshot.version, &snapshot.text)
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
            range_formatting_edits_for_text(uri.as_str(), snapshot.version, &snapshot.text, range)
        }))
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = params.text_document.uri;
        let documents = self.documents.read().await;
        Ok(documents.get(&uri).map(|snapshot| {
            let core = CompilerCore;
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
        completion_provider: Some(CompletionOptions::default()),
        hover_provider: Some(tower_lsp::lsp_types::HoverProviderCapability::Simple(true)),
        definition_provider: Some(OneOf::Left(true)),
        references_provider: Some(OneOf::Left(true)),
        document_formatting_provider: Some(OneOf::Left(true)),
        document_range_formatting_provider: Some(OneOf::Left(true)),
        code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
        ..ServerCapabilities::default()
    }
}
