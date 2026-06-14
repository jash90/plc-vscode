use plc_lsp_server::{document_symbols_for_text, server_capabilities};
use tower_lsp::lsp_types::{OneOf, SymbolKind};

#[test]
fn lsp_server_advertises_document_symbol_support() {
    assert_eq!(
        server_capabilities().document_symbol_provider,
        Some(OneOf::Left(true))
    );
}

#[test]
fn lsp_server_maps_compiler_core_document_symbols() {
    let symbols = document_symbols_for_text(
        "file:///main.st",
        1,
        "PROGRAM Main\nVAR\n    Enabled : BOOL;\nEND_VAR\nEND_PROGRAM\n",
    );

    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].name, "Main");
    assert_eq!(symbols[0].kind, SymbolKind::MODULE);
    let children = symbols[0]
        .children
        .as_ref()
        .expect("variable child symbols");
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].name, "Enabled");
    assert_eq!(children[0].detail.as_deref(), Some("BOOL"));
    assert_eq!(children[0].kind, SymbolKind::VARIABLE);
}
