use plc_compiler_core::{CompilerCore, SourceDocument};

#[test]
fn executes_minimal_program_and_reports_initialized_string_state() {
    let core = CompilerCore::default();
    let document = SourceDocument::new(
        "file:///hello.st",
        1,
        "PROGRAM Hello\nVAR\n    Message : STRING := 'Hello from standard ST';\nEND_VAR\nEND_PROGRAM\n",
    );

    let result = core.execute(&document);

    assert!(result.diagnostics().is_empty());
    assert_eq!(result.output(), &["Message = Hello from standard ST"]);
}

#[test]
fn execution_returns_diagnostics_instead_of_output_for_invalid_program() {
    let core = CompilerCore::default();
    let document = SourceDocument::new(
        "file:///bad.st",
        1,
        "PROGRAM Bad\nVAR\n    Message : STRING := 'No terminator';\nEND_VAR",
    );

    let result = core.execute(&document);

    assert!(!result.diagnostics().is_empty());
    assert!(result.output().is_empty());
}
