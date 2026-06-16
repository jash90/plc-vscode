//! Semantic-layer coverage derived from `PRG_Test_ST.st` — symbol indexing,
//! type model, and diagnostics on the real program.

use plc_semantics::{SymbolKind, TypeKind, analyze_file};

const FIXTURE: &str = include_str!("fixtures/prg_test_st.st");
const URI: &str = "file:///prg_test.st";

fn type_of(var: &str) -> TypeKind {
    let analysis = analyze_file(URI, FIXTURE);
    analysis
        .symbol_index
        .find_in_container("PRG_Test", var)
        .unwrap_or_else(|| panic!("variable {var} not indexed"))
        .type_kind
        .clone()
        .unwrap_or_else(|| panic!("variable {var} has no type"))
}

#[test]
fn prg_test_is_indexed_as_program_symbol() {
    let analysis = analyze_file(URI, FIXTURE);
    let program = analysis
        .symbol_index
        .find_top_level("PRG_Test")
        .expect("PRG_Test symbol");
    assert_eq!(program.kind, SymbolKind::Program);
    assert!(program.container.is_none());
}

#[test]
fn log_variables_are_string_typed_in_prg_test() {
    for name in ["sLog01", "sLog15", "sLog30"] {
        let analysis = analyze_file(URI, FIXTURE);
        let symbol = analysis
            .symbol_index
            .find_in_container("PRG_Test", name)
            .unwrap();
        assert_eq!(symbol.kind, SymbolKind::Variable);
        assert_eq!(symbol.type_kind, Some(TypeKind::String));
        assert_eq!(symbol.container.as_deref(), Some("PRG_Test"));
    }
}

#[test]
fn integer_variables_indexed_as_integer() {
    for name in ["iA", "iB", "iWynik", "i", "iDlugosc"] {
        assert_eq!(type_of(name), TypeKind::Integer, "{name}");
    }
}

#[test]
fn dint_variables_indexed_as_integer() {
    for name in ["diWynik", "iSuma", "iSilnia"] {
        assert_eq!(type_of(name), TypeKind::Integer, "{name}");
    }
}

#[test]
fn real_variables_indexed_as_real() {
    for name in ["rA", "rB", "rWynik"] {
        assert_eq!(type_of(name), TypeKind::Real, "{name}");
    }
}

#[test]
fn bool_variables_indexed_as_bool() {
    for name in ["xA", "xB", "xWynik", "xWejscie", "xTakt", "xInit"] {
        assert_eq!(type_of(name), TypeKind::Bool, "{name}");
    }
}

#[test]
fn time_variables_indexed_as_time() {
    for name in ["tA", "tB", "tWynik"] {
        assert_eq!(type_of(name), TypeKind::Time, "{name}");
    }
}

#[test]
fn word_variables_indexed_as_bit_string_16() {
    for name in ["wA", "wB", "wWynik"] {
        assert_eq!(type_of(name), TypeKind::BitString(16), "{name}");
    }
}

#[test]
fn string_working_variables_indexed_as_string() {
    for name in ["sImie", "sNazw", "sTekst"] {
        assert_eq!(type_of(name), TypeKind::String, "{name}");
    }
}

#[test]
fn function_block_instances_indexed_as_derived_typed_variables() {
    let analysis = analyze_file(URI, FIXTURE);
    let ton = analysis
        .symbol_index
        .find_in_container("PRG_Test", "fbTON")
        .unwrap();
    assert_eq!(ton.kind, SymbolKind::Variable);
    assert_eq!(ton.type_kind, Some(TypeKind::Derived("TON".to_owned())));
    let ctu = analysis
        .symbol_index
        .find_in_container("PRG_Test", "fbCTU")
        .unwrap();
    assert_eq!(ctu.type_kind, Some(TypeKind::Derived("CTU".to_owned())));
}

#[test]
fn full_fixture_yields_zero_semantic_diagnostics() {
    let analysis = analyze_file(URI, FIXTURE);
    assert!(
        analysis.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        analysis.diagnostics
    );
}

#[test]
fn no_unresolved_or_mismatch_codes_on_real_file() {
    let analysis = analyze_file(URI, FIXTURE);
    assert!(analysis.diagnostics.iter().all(|d| d.code != "SEM0001"));
    assert!(analysis.diagnostics.iter().all(|d| d.code != "SEM0002"));
}

#[test]
fn word_assigned_from_call_expression_is_not_flagged() {
    let src =
        "PROGRAM P\nVAR\n wA : WORD; wWynik : WORD;\nEND_VAR\nwWynik := SHL(wA, 4);\nEND_PROGRAM\n";
    assert!(analyze_file(URI, src).diagnostics.is_empty());
}

#[test]
fn word_assigned_real_is_flagged_with_word_in_message() {
    let src = "PROGRAM P\nVAR\n wWynik : WORD; rA : REAL;\nEND_VAR\nwWynik := rA;\nEND_PROGRAM\n";
    let analysis = analyze_file(URI, src);
    let mismatch = analysis
        .diagnostics
        .iter()
        .find(|d| d.code == "SEM0002")
        .expect("a SEM0002 mismatch");
    assert!(
        mismatch.message.contains("WORD"),
        "message: {}",
        mismatch.message
    );
}

#[test]
fn real_target_accepts_integer_widening() {
    let src =
        "PROGRAM P\nVAR\n rWynik : REAL; iA : INT := 2;\nEND_VAR\nrWynik := iA;\nEND_PROGRAM\n";
    assert!(analyze_file(URI, src).diagnostics.is_empty());
}

#[test]
fn bool_target_rejects_string_literal() {
    let src = "PROGRAM P\nVAR\n xWynik : BOOL;\nEND_VAR\nxWynik := 'TRUE';\nEND_PROGRAM\n";
    let analysis = analyze_file(URI, src);
    assert!(
        analysis
            .diagnostics
            .iter()
            .any(|d| d.code == "SEM0002" && d.message.contains("BOOL"))
    );
}

#[test]
fn type_model_nuances_used_by_the_file() {
    assert_eq!(TypeKind::from_name("DINT"), TypeKind::Integer);
    assert_eq!(TypeKind::from_name("WORD"), TypeKind::BitString(16));
    assert_eq!(
        TypeKind::from_name("TON"),
        TypeKind::Derived("TON".to_owned())
    );
    assert_eq!(TypeKind::Integer.display_name(), "integer");
    assert_eq!(TypeKind::Real.display_name(), "real");
    assert_eq!(TypeKind::Time.display_name(), "time/date");
    assert_eq!(TypeKind::BitString(16).display_name(), "WORD");
    assert!(!TypeKind::BitString(16).assignment_compatible(&TypeKind::Real));
    assert!(TypeKind::BitString(16).assignment_compatible(&TypeKind::Integer));
}
