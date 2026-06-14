//! LSP server implementation for PLC VS Code.

use plc_compiler_core::{
    CompilerCore, DiagnosticSeverity as CoreSeverity, DocumentSymbol as CoreDocumentSymbol,
    Range as CoreRange, SourceDocument, SymbolKind as CoreSymbolKind,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse, InitializeParams,
    InitializeResult, MessageType, OneOf, Position, Range, ServerCapabilities, SymbolKind,
    TextDocumentSyncCapability, TextDocumentSyncKind, Url,
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
        let text = params
            .content_changes
            .into_iter()
            .next()
            .map(|change| change.text)
            .unwrap_or_default();

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
}

/// Server capabilities helper used by tests and by `initialize`.
pub fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        document_symbol_provider: Some(OneOf::Left(true)),
        ..ServerCapabilities::default()
    }
}
