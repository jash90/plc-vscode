use plc_lsp_server::diagnostics_for_text;

// Regression guard for PLC-29: an empty/no-content change must not be treated
// as valid source. The server preserves the last known text; this unit-level
// check ensures the diagnostics path still reports a real error for the stored
// document text rather than silently clearing diagnostics on empty input.
#[test]
fn diagnostics_are_reported_for_incomplete_program_text() {
    let diagnostics = diagnostics_for_text("file:///main.st", 2, "PROGRAM Main\nVAR\nEND_VAR\n");
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("END_PROGRAM"));
}

#[test]
fn empty_text_produces_no_false_program_diagnostic() {
    let diagnostics = diagnostics_for_text("file:///main.st", 1, "");
    assert!(diagnostics.is_empty());
}
