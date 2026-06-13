//! LSP server implementation for PLC VS Code.

use plc_compiler_core::{CompilerCore, DiagnosticSeverity as CoreSeverity, SourceDocument};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    InitializeParams, InitializeResult, MessageType, Position, Range, ServerCapabilities,
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
            range: Range {
                start: Position {
                    line: diagnostic.range.start.line,
                    character: diagnostic.range.start.character,
                },
                end: Position {
                    line: diagnostic.range.end.line,
                    character: diagnostic.range.end.character,
                },
            },
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
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..ServerCapabilities::default()
            },
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
}

/// Server capabilities helper used by tests and by `initialize`.
pub fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        ..ServerCapabilities::default()
    }
}
