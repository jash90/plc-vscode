use plc_lsp_server::diagnostics_for_text;
use tower_lsp::lsp_types::DiagnosticSeverity;

#[test]
fn lsp_server_maps_compiler_core_diagnostics() {
    let diagnostics = diagnostics_for_text("file:///main.st", 1, "PROGRAM Main\nVAR\nEND_VAR\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
    assert!(diagnostics[0].message.contains("END_PROGRAM"));
    assert_eq!(diagnostics[0].source.as_deref(), Some("plc-vscode"));
}

#[test]
fn lsp_server_returns_empty_diagnostics_for_valid_minimal_program() {
    let diagnostics = diagnostics_for_text(
        "file:///main.st",
        1,
        "PROGRAM Main\nVAR\nEND_VAR\nEND_PROGRAM\n",
    );

    assert!(diagnostics.is_empty());
}
