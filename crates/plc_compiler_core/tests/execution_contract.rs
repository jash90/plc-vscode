use plc_compiler_core::{CompilerCore, SourceDocument};

#[test]
fn executes_minimal_program_with_plc_print() {
    let core = CompilerCore::default();
    let document = SourceDocument::new(
        "file:///hello.st",
        1,
        "PROGRAM Hello\nVAR\nEND_VAR\nPLC_PRINT('Hello from ST');\nEND_PROGRAM\n",
    );

    let result = core.execute(&document);

    assert!(result.diagnostics().is_empty());
    assert_eq!(result.output(), &["Hello from ST"]);
}

#[test]
fn execution_returns_diagnostics_instead_of_output_for_invalid_program() {
    let core = CompilerCore::default();
    let document = SourceDocument::new(
        "file:///bad.st",
        1,
        "PROGRAM Bad\nPLC_PRINT('No terminator');",
    );

    let result = core.execute(&document);

    assert!(!result.diagnostics().is_empty());
    assert!(result.output().is_empty());
}
