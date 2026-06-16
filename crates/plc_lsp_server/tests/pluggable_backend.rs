//! Proves the provided tower-lsp server accepts a third-party analysis backend:
//! any `plc_api::LanguageService` can be plugged in via `with_service`, and the
//! trait is object-safe (`Arc<dyn LanguageService + Send + Sync>`).

use std::sync::Arc;

use plc_api::{
    Analysis, CodeAction, CompletionCandidate, Diagnostic, DiagnosticSeverity, ExecutionResult,
    HoverInfo, LanguageService, Location, Position, Range, SemanticToken, SignatureInfo,
    SourceDocument, SymbolAnalysis, TextEdit, WorkspaceSymbol,
};
use plc_lsp_server::PlcLanguageServer;

/// A trivial alternate analyzer (e.g. a third party's own engine) that emits one
/// canned diagnostic and otherwise returns empty results.
struct StubAnalyzer;

impl LanguageService for StubAnalyzer {
    fn analyze(&self, document: &SourceDocument) -> Analysis {
        Analysis::new(
            document.uri().to_owned(),
            document.version(),
            vec![Diagnostic {
                severity: DiagnosticSeverity::Error,
                range: Range::at_start(),
                code: "STUB001",
                message: "stub diagnostic from a third-party backend".to_owned(),
            }],
        )
    }
    fn execute(&self, _: &SourceDocument) -> ExecutionResult {
        ExecutionResult::new(Vec::new(), Vec::new())
    }
    fn document_symbols(&self, document: &SourceDocument) -> SymbolAnalysis {
        SymbolAnalysis::new(document.uri().to_owned(), document.version(), Vec::new())
    }
    fn workspace_symbols(&self, _: &[SourceDocument], _: &str) -> Vec<WorkspaceSymbol> {
        Vec::new()
    }
    fn semantic_tokens(&self, _: &SourceDocument) -> Vec<SemanticToken> {
        Vec::new()
    }
    fn completions(&self, _: &SourceDocument, _: Position) -> Vec<CompletionCandidate> {
        Vec::new()
    }
    fn hover(&self, _: &SourceDocument, _: Position) -> Option<HoverInfo> {
        None
    }
    fn signature_help(&self, _: &SourceDocument, _: Position) -> Option<SignatureInfo> {
        None
    }
    fn definition(&self, _: &SourceDocument, _: Position) -> Option<Location> {
        None
    }
    fn references(&self, _: &SourceDocument, _: Position, _: bool) -> Vec<Location> {
        Vec::new()
    }
    fn formatting(&self, _: &SourceDocument) -> Vec<TextEdit> {
        Vec::new()
    }
    fn formatting_range(&self, _: &SourceDocument, _: Range) -> Vec<TextEdit> {
        Vec::new()
    }
    fn code_actions(&self, _: &SourceDocument) -> Vec<CodeAction> {
        Vec::new()
    }
}

#[test]
fn language_service_trait_is_object_safe() {
    // Fails to compile if the port is not dyn-compatible.
    let service: Arc<dyn LanguageService + Send + Sync> = Arc::new(StubAnalyzer);
    let document = SourceDocument::new("file:///x.st", 1, "PROGRAM Main\nEND_PROGRAM\n");
    assert_eq!(service.analyze(&document).diagnostics()[0].code, "STUB001");
}

#[test]
fn provided_server_accepts_a_third_party_backend() {
    // Wire the custom backend into the PROVIDED tower-lsp host (capabilities,
    // delta token encoding, document store all reused). The stock server is the
    // same call with `Arc::new(CompilerCore)`.
    let (_service, _socket) = tower_lsp::LspService::new(|client| {
        PlcLanguageServer::with_service(client, Arc::new(StubAnalyzer))
    });
}

#[test]
fn provided_server_can_be_language_aware_via_a_registry() {
    // The server can pick its analyzer per document by extension (ST/IL/…).
    let registry = Arc::new(plc_lang::LanguageRegistry::with_builtins());
    let (_service, _socket) = tower_lsp::LspService::new(move |client| {
        PlcLanguageServer::with_registry(client, Arc::clone(&registry))
    });
}
