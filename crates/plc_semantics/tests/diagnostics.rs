use plc_semantics::analyze_file;

#[test]
fn reports_unresolved_assignment_targets() {
    let analysis = analyze_file(
        "file:///main.st",
        "PROGRAM Main\nMissing := TRUE;\nEND_PROGRAM\n",
    );

    assert_eq!(analysis.diagnostics.len(), 1);
    assert_eq!(analysis.diagnostics[0].code, "SEM0001");
    assert!(analysis.diagnostics[0].message.contains("Missing"));
}

#[test]
fn resolves_declared_assignment_targets() {
    let analysis = analyze_file(
        "file:///main.st",
        "PROGRAM Main\nVAR\n    Enabled : BOOL;\nEND_VAR\nEnabled := TRUE;\nEND_PROGRAM\n",
    );

    assert!(analysis.diagnostics.is_empty());
}
