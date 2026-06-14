use plc_lsp_server::server_capabilities;
use tower_lsp::lsp_types::{OneOf, TextDocumentSyncCapability, TextDocumentSyncKind};

// Protocol baseline integration tests (PLC-46): assert the advertised server
// capabilities form a stable contract for clients across the implemented
// feature set (sync mode + the providers wired by the IDE feature tasks).
#[test]
fn advertises_full_text_document_sync() {
    let capabilities = server_capabilities();
    assert!(matches!(
        capabilities.text_document_sync,
        Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL))
    ));
}

#[test]
fn advertises_implemented_feature_providers() {
    let capabilities = server_capabilities();
    assert_eq!(
        capabilities.document_symbol_provider,
        Some(OneOf::Left(true))
    );
    assert!(capabilities.completion_provider.is_some());
    assert!(capabilities.hover_provider.is_some());
}
