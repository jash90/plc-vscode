use plc_lsp_server::{server_capabilities, workspace_symbols_for_documents};
use tower_lsp::lsp_types::{OneOf, SymbolKind};

// Workspace symbol protocol tests (PLC-58): assert the advertised capability
// and cross-file lookup behavior of the workspace/symbol provider.

fn sample_workspace() -> Vec<(String, String)> {
    vec![
        (
            "file:///a.st".to_owned(),
            "PROGRAM Main\nEND_PROGRAM\n".to_owned(),
        ),
        (
            "file:///b.st".to_owned(),
            "FUNCTION_BLOCK Motor\nEND_FUNCTION_BLOCK\n".to_owned(),
        ),
    ]
}

#[test]
fn lsp_server_advertises_workspace_symbol_support() {
    assert_eq!(
        server_capabilities().workspace_symbol_provider,
        Some(OneOf::Left(true))
    );
}

#[test]
fn lsp_server_workspace_symbol_query_filters_across_files() {
    let documents = sample_workspace();

    let symbols = workspace_symbols_for_documents(&documents, "Motor");

    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].name, "Motor");
    assert_eq!(symbols[0].kind, SymbolKind::CLASS);
    assert_eq!(symbols[0].location.uri.as_str(), "file:///b.st");
}

#[test]
fn lsp_server_workspace_symbol_returns_all_top_level_for_empty_query() {
    let documents = sample_workspace();

    let symbols = workspace_symbols_for_documents(&documents, "");

    assert!(symbols.iter().any(|symbol| symbol.name == "Main"));
    assert!(symbols.iter().any(|symbol| symbol.name == "Motor"));
}
