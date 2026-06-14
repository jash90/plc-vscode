use plc_compiler_core::{CompilerCore, DiagnosticSeverity, SourceDocument};

#[test]
fn compiler_core_uses_syntax_ranges_for_diagnostics() {
    let core = CompilerCore::default();
    let document = SourceDocument::new(
        "file:///main.st",
        1,
        "// banner\nPROGRAM Main\nVAR\nEND_VAR\n",
    );

    let analysis = core.analyze(&document);

    assert_eq!(analysis.diagnostics().len(), 1);
    assert_eq!(analysis.diagnostics()[0].code, "PLC0002");
    assert_eq!(analysis.diagnostics()[0].range.start.line, 1);
    assert_eq!(analysis.diagnostics()[0].range.start.character, 0);
}

#[test]
fn compiler_core_surfaces_semantic_diagnostics() {
    let core = CompilerCore::default();
    let document = SourceDocument::new(
        "file:///main.st",
        1,
        "PROGRAM Main\nVAR\n    Enabled : BOOL;\nEND_VAR\nEnabled := 'yes';\nEND_PROGRAM\n",
    );

    let analysis = core.analyze(&document);

    assert_eq!(analysis.diagnostics().len(), 1);
    assert_eq!(analysis.diagnostics()[0].code, "SEM0002");
    assert!(analysis.diagnostics()[0].message.contains("BOOL"));
}

#[test]
fn compiler_core_analyzes_text_and_returns_versioned_diagnostics() {
    let core = CompilerCore::default();
    let document = SourceDocument::new("file:///main.st", 7, "PROGRAM Main\nVAR\nEND_VAR\n");

    let analysis = core.analyze(&document);

    assert_eq!(analysis.uri(), "file:///main.st");
    assert_eq!(analysis.version(), 7);
    assert_eq!(analysis.diagnostics().len(), 1);
    assert_eq!(
        analysis.diagnostics()[0].severity,
        DiagnosticSeverity::Error
    );
    assert!(analysis.diagnostics()[0].message.contains("END_PROGRAM"));
}

#[test]
fn compiler_core_returns_no_diagnostics_for_minimal_program() {
    let core = CompilerCore::default();
    let document = SourceDocument::new(
        "file:///main.st",
        1,
        "PROGRAM Main\nVAR\nEND_VAR\nEND_PROGRAM\n",
    );

    let analysis = core.analyze(&document);

    assert!(analysis.diagnostics().is_empty());
}

#[test]
fn compiler_core_detects_unclosed_block_comments() {
    let core = CompilerCore::default();
    let document = SourceDocument::new(
        "file:///main.st",
        1,
        "PROGRAM Main\n(* unfinished\nEND_PROGRAM",
    );

    let analysis = core.analyze(&document);

    assert_eq!(analysis.diagnostics().len(), 1);
    assert!(
        analysis.diagnostics()[0]
            .message
            .contains("Unclosed block comment")
    );
}
