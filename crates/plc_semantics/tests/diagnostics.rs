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
fn reports_basic_assignment_type_mismatches() {
    let analysis = analyze_file(
        "file:///main.st",
        "PROGRAM Main\nVAR\n    Enabled : BOOL;\nEND_VAR\nEnabled := 'yes';\nEND_PROGRAM\n",
    );

    assert_eq!(analysis.diagnostics.len(), 1);
    assert_eq!(analysis.diagnostics[0].code, "SEM0002");
    assert!(analysis.diagnostics[0].message.contains("BOOL"));
}

#[test]
fn accepts_assignment_between_compatible_variables() {
    let analysis = analyze_file(
        "file:///main.st",
        "PROGRAM Main\nVAR\n    Source : BOOL;\n    Target : BOOL;\nEND_VAR\nTarget := Source;\nEND_PROGRAM\n",
    );

    assert!(analysis.diagnostics.is_empty());
}
