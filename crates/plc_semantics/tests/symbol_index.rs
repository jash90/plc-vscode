use plc_semantics::{SourceFile, SymbolKind, TypeKind, analyze_workspace};

#[test]
fn indexes_pous_and_local_variables() {
    let analysis = analyze_workspace(&[SourceFile::new(
        "file:///main.st",
        "PROGRAM Main\nVAR\n    Enabled : BOOL;\n    Count : INT;\nEND_VAR\nEND_PROGRAM\n",
    )]);

    assert!(analysis.diagnostics.is_empty());
    assert!(
        analysis
            .symbol_index
            .symbols()
            .iter()
            .any(|symbol| symbol.name == "Main" && symbol.kind == SymbolKind::Program)
    );
    let enabled = analysis
        .symbol_index
        .find_in_container("Main", "Enabled")
        .expect("Enabled variable should be indexed");
    assert_eq!(enabled.kind, SymbolKind::Variable);
    assert_eq!(enabled.type_kind, Some(TypeKind::Bool));
}

#[test]
fn indexes_cross_file_top_level_symbols_deterministically() {
    let analysis = analyze_workspace(&[
        SourceFile::new("file:///a.st", "PROGRAM Main\nEND_PROGRAM\n"),
        SourceFile::new("file:///b.st", "FUNCTION_BLOCK Motor\nEND_FUNCTION_BLOCK\n"),
    ]);

    assert!(analysis.symbol_index.find_top_level("Main").is_some());
    assert!(analysis.symbol_index.find_top_level("Motor").is_some());
}
