//! ST -> IL conversion coverage derived from `PRG_Test_ST.st` statements, plus
//! whole-file fidelity behavior. Complements `conversion_st_il.rs` (which uses a
//! single counter program).

use plc_api::SourceDocument;
use plc_lang::LanguageRegistry;

const FIXTURE: &str = include_str!("fixtures/prg_test_st.st");

/// Convert a small ST program to IL and return its trimmed non-empty lines.
fn il_lines(st: &str) -> Vec<String> {
    let registry = LanguageRegistry::with_builtins();
    let document = SourceDocument::new("file:///p.st", 0, st);
    let out = registry.convert("st", "il", &document);
    assert!(out.error.is_none(), "convert error: {:?}", out.error);
    out.text
        .lines()
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty())
        .collect()
}

fn program(body: &str, vars: &str) -> String {
    format!("PROGRAM P\nVAR\n{vars}\nEND_VAR\n{body}\nEND_PROGRAM\n")
}

#[test]
fn addition_statement_lowers_to_ld_add_st() {
    let lines = il_lines(&program(
        "iWynik := iA + iB;",
        " iA:INT:=2; iB:INT:=3; iWynik:INT;",
    ));
    assert!(lines.contains(&"LD iA".to_owned()), "{lines:?}");
    assert!(lines.contains(&"ADD iB".to_owned()), "{lines:?}");
    assert!(lines.contains(&"ST iWynik".to_owned()), "{lines:?}");
}

#[test]
fn subtraction_statement_lowers_to_sub() {
    let lines = il_lines(&program(
        "iWynik := iB - iA;",
        " iA:INT:=2; iB:INT:=3; iWynik:INT;",
    ));
    assert!(lines.contains(&"LD iB".to_owned()), "{lines:?}");
    assert!(lines.contains(&"SUB iA".to_owned()), "{lines:?}");
    assert!(lines.contains(&"ST iWynik".to_owned()), "{lines:?}");
}

#[test]
fn unsupported_operator_becomes_one_opaque_load() {
    // `*` is not modeled by the IR, so the whole RHS becomes one operand.
    let lines = il_lines(&program(
        "iWynik := iA * iB;",
        " iA:INT:=2; iB:INT:=3; iWynik:INT;",
    ));
    assert!(lines.contains(&"LD iA * iB".to_owned()), "{lines:?}");
    assert!(lines.contains(&"ST iWynik".to_owned()), "{lines:?}");
    assert!(
        !lines
            .iter()
            .any(|l| l.starts_with("ADD") || l.starts_with("SUB"))
    );
}

#[test]
fn real_literal_operand_keeps_decimal_point() {
    let lines = il_lines(&program(
        "rWynik := rA + 2.0;",
        " rA:REAL:=7.0; rWynik:REAL;",
    ));
    assert!(lines.contains(&"LD rA".to_owned()), "{lines:?}");
    assert!(lines.contains(&"ADD 2.0".to_owned()), "{lines:?}");
}

#[test]
fn var_block_renders_types_word_maps_to_int() {
    let lines = il_lines(&program("wWynik := wA;", " wA:WORD; tA:TIME; wWynik:WORD;"));
    assert!(
        lines.contains(&"wA : INT;".to_owned()),
        "WORD should render as INT: {lines:?}"
    );
    assert!(lines.contains(&"tA : TIME;".to_owned()), "{lines:?}");
}

#[test]
fn multi_statement_program_emits_each_assignment() {
    let lines = il_lines(&program(
        "iWynik := iA + iB;\niWynik := iB - iA;",
        " iA:INT:=2; iB:INT:=3; iWynik:INT;",
    ));
    assert_eq!(lines.iter().filter(|l| *l == "ST iWynik").count(), 2);
    assert!(lines.contains(&"ADD iB".to_owned()));
    assert!(lines.contains(&"SUB iA".to_owned()));
}

#[test]
fn st_to_st_identity_renders_infix_expression() {
    let registry = LanguageRegistry::with_builtins();
    let document = SourceDocument::new(
        "file:///p.st",
        0,
        program("iWynik := iA + iB;", " iA:INT:=2; iB:INT:=3; iWynik:INT;"),
    );
    let out = registry.convert("st", "st", &document);
    assert!(out.error.is_none());
    assert!(out.text.contains("iWynik := iA + iB;"), "{}", out.text);
}

#[test]
fn full_fixture_converts_with_fidelity_notes_and_no_panic() {
    let registry = LanguageRegistry::with_builtins();
    let document = SourceDocument::new("file:///prg.st", 0, FIXTURE);
    let out = registry.convert("st", "il", &document);
    // The whole file analyzes clean, so conversion proceeds...
    assert!(out.error.is_none(), "error: {:?}", out.error);
    // ...but control-flow / FB-call statements are not modeled -> fidelity notes.
    assert!(!out.fidelity.is_empty(), "expected dropped-statement notes");
    // The assignment targets still appear as `ST <target>` (operand spelling may
    // be garbled for call RHS — assert only the store side).
    assert!(
        out.text.lines().any(|l| l.trim() == "ST sLog01"),
        "no ST sLog01"
    );
}

#[test]
fn full_fixture_il_back_to_st_does_not_error() {
    let registry = LanguageRegistry::with_builtins();
    let st_doc = SourceDocument::new("file:///prg.st", 0, FIXTURE);
    let il = registry.convert("st", "il", &st_doc).text;
    let il_doc = SourceDocument::new("file:///prg.il", 0, il);
    let back = registry.convert("il", "st", &il_doc);
    assert!(back.error.is_none(), "il->st error: {:?}", back.error);
    assert!(back.text.contains("PROGRAM"));
}
