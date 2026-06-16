//! Parser coverage derived from `PRG_Test_ST.st` — POU shape, VAR declarations,
//! and statement-kind recognition.

use plc_syntax::{
    DeclarationBlock, PouKind, StatementKind, VarBlockKind, VariableDeclaration, parse_source,
};

const FIXTURE: &str = include_str!("fixtures/prg_test_st.st");

fn decls(blocks: &[DeclarationBlock]) -> Vec<&VariableDeclaration> {
    blocks.iter().flat_map(|b| b.declarations.iter()).collect()
}

fn decl<'a>(blocks: &'a [DeclarationBlock], name: &str) -> &'a VariableDeclaration {
    decls(blocks)
        .into_iter()
        .find(|d| d.name == name)
        .unwrap_or_else(|| panic!("declaration {name} not found"))
}

#[test]
fn parses_single_program_named_prg_test() {
    let parse = parse_source(FIXTURE);
    assert_eq!(parse.units().len(), 1);
    assert_eq!(parse.units()[0].kind, PouKind::Program);
    assert_eq!(parse.units()[0].name.as_deref(), Some("PRG_Test"));
}

#[test]
fn full_fixture_parses_without_diagnostics() {
    assert!(parse_source(FIXTURE).diagnostics().is_empty());
}

#[test]
fn single_var_block() {
    let parse = parse_source(FIXTURE);
    let blocks = &parse.units()[0].declaration_blocks;
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].kind, VarBlockKind::Var);
}

#[test]
fn declares_thirty_sized_string_log_variables() {
    let parse = parse_source(FIXTURE);
    let blocks = &parse.units()[0].declaration_blocks;
    for n in 1..=30 {
        let name = format!("sLog{n:02}");
        let d = decl(blocks, &name);
        assert_eq!(d.type_name, "STRING", "{name} type");
        assert_eq!(d.type_size.as_deref(), Some("[80]"), "{name} size");
        assert_eq!(d.initializer, None, "{name} init");
    }
}

#[test]
fn captures_int_and_real_initializers() {
    let parse = parse_source(FIXTURE);
    let blocks = &parse.units()[0].declaration_blocks;
    assert_eq!(decl(blocks, "iA").type_name, "INT");
    assert_eq!(decl(blocks, "iA").initializer.as_deref(), Some("2"));
    assert_eq!(decl(blocks, "iB").initializer.as_deref(), Some("3"));
    assert_eq!(decl(blocks, "rA").type_name, "REAL");
    assert_eq!(decl(blocks, "rA").initializer.as_deref(), Some("7.0"));
    assert_eq!(decl(blocks, "rB").initializer.as_deref(), Some("2.0"));
}

#[test]
fn captures_word_hex_initializers() {
    let parse = parse_source(FIXTURE);
    let blocks = &parse.units()[0].declaration_blocks;
    assert_eq!(decl(blocks, "wA").type_name, "WORD");
    assert_eq!(decl(blocks, "wA").type_size, None);
    assert_eq!(decl(blocks, "wA").initializer.as_deref(), Some("16#000F"));
    assert_eq!(decl(blocks, "wB").initializer.as_deref(), Some("16#00F0"));
    assert_eq!(decl(blocks, "wWynik").type_name, "WORD");
    assert_eq!(decl(blocks, "wWynik").initializer, None);
}

#[test]
fn captures_time_duration_initializers() {
    let parse = parse_source(FIXTURE);
    let blocks = &parse.units()[0].declaration_blocks;
    assert_eq!(decl(blocks, "tA").type_name, "TIME");
    assert_eq!(decl(blocks, "tA").initializer.as_deref(), Some("T#1h30m"));
    assert_eq!(decl(blocks, "tB").initializer.as_deref(), Some("T#45m"));
    assert_eq!(decl(blocks, "tWynik").type_name, "TIME");
    assert_eq!(decl(blocks, "tWynik").initializer, None);
}

#[test]
fn captures_bool_initializers() {
    let parse = parse_source(FIXTURE);
    let blocks = &parse.units()[0].declaration_blocks;
    assert_eq!(decl(blocks, "xA").type_name, "BOOL");
    assert_eq!(decl(blocks, "xA").initializer.as_deref(), Some("TRUE"));
    assert_eq!(decl(blocks, "xB").initializer.as_deref(), Some("FALSE"));
    assert_eq!(decl(blocks, "xInit").initializer.as_deref(), Some("TRUE"));
}

#[test]
fn captures_sized_string_name_initializers() {
    let parse = parse_source(FIXTURE);
    let blocks = &parse.units()[0].declaration_blocks;
    assert_eq!(decl(blocks, "sImie").type_name, "STRING");
    assert_eq!(decl(blocks, "sImie").type_size.as_deref(), Some("[20]"));
    assert_eq!(decl(blocks, "sImie").initializer.as_deref(), Some("'Jan'"));
    assert_eq!(
        decl(blocks, "sNazw").initializer.as_deref(),
        Some("'Kowalski'")
    );
}

#[test]
fn function_block_instances_use_derived_type_names() {
    let parse = parse_source(FIXTURE);
    let blocks = &parse.units()[0].declaration_blocks;
    assert_eq!(decl(blocks, "fbTON").type_name, "TON");
    assert_eq!(decl(blocks, "fbTON").type_size, None);
    assert_eq!(decl(blocks, "fbTON").initializer, None);
    assert_eq!(decl(blocks, "fbCTU").type_name, "CTU");
}

#[test]
fn statement_kinds_present_and_counts() {
    let parse = parse_source(FIXTURE);
    let kinds: Vec<StatementKind> = parse.units()[0].statements.iter().map(|s| s.kind).collect();
    let count = |k: StatementKind| kinds.iter().filter(|&&x| x == k).count();
    assert!(count(StatementKind::Assignment) > 0);
    assert_eq!(count(StatementKind::If), 1);
    assert_eq!(count(StatementKind::Case), 1);
    assert_eq!(count(StatementKind::For), 2);
    assert_eq!(count(StatementKind::While), 1);
}

#[test]
fn statement_kinds_absent() {
    let parse = parse_source(FIXTURE);
    let kinds: Vec<StatementKind> = parse.units()[0].statements.iter().map(|s| s.kind).collect();
    for absent in [
        StatementKind::Repeat,
        StatementKind::Return,
        StatementKind::Exit,
        StatementKind::Continue,
    ] {
        assert!(!kinds.contains(&absent), "{absent:?} should be absent");
    }
}

#[test]
fn first_assignment_captures_target_and_expression() {
    let parse = parse_source(FIXTURE);
    let first = parse.units()[0]
        .statements
        .iter()
        .find(|s| s.kind == StatementKind::Assignment)
        .expect("an assignment");
    assert_eq!(first.target.as_deref(), Some("iWynik"));
    let expr = first.expression.as_deref().unwrap();
    assert!(
        expr.contains("iA") && expr.contains("iB") && expr.contains('+'),
        "expr was {expr:?}"
    );
}

#[test]
fn function_block_call_is_not_an_assignment() {
    let src = "PROGRAM P\nVAR\n fbTON : TON;\n xWejscie : BOOL;\nEND_VAR\nfbTON(IN := xWejscie, PT := T#2s);\nEND_PROGRAM\n";
    let parse = parse_source(src);
    let assignments: Vec<_> = parse.units()[0]
        .statements
        .iter()
        .filter(|s| s.kind == StatementKind::Assignment)
        .collect();
    assert!(
        assignments
            .iter()
            .all(|s| s.target.as_deref() != Some("fbTON"))
    );
    assert!(
        assignments
            .iter()
            .all(|s| s.target.as_deref() != Some("IN"))
    );
    assert!(
        assignments
            .iter()
            .all(|s| s.target.as_deref() != Some("PT"))
    );
}

#[test]
fn member_access_target_is_not_captured_as_assignment() {
    let src = "PROGRAM P\nVAR\n sLog29 : STRING[80]; fbTON : TON;\nEND_VAR\nsLog29 := TIME_TO_STRING(fbTON.ET);\nEND_PROGRAM\n";
    let parse = parse_source(src);
    let assignments: Vec<_> = parse.units()[0]
        .statements
        .iter()
        .filter(|s| s.kind == StatementKind::Assignment)
        .collect();
    assert!(
        assignments
            .iter()
            .any(|s| s.target.as_deref() == Some("sLog29"))
    );
    assert!(
        assignments
            .iter()
            .all(|s| s.target.as_deref() != Some("ET"))
    );
}

#[test]
fn while_loop_does_not_swallow_following_statement() {
    let src = "PROGRAM P\nVAR\n iWynik : INT; sLog27 : STRING[80];\nEND_VAR\nWHILE iWynik <= 100 DO\n iWynik := iWynik * 2;\nEND_WHILE\nsLog27 := 'x';\nEND_PROGRAM\n";
    let parse = parse_source(src);
    let statements = &parse.units()[0].statements;
    assert!(statements.iter().any(|s| s.kind == StatementKind::While));
    assert!(
        statements
            .iter()
            .any(|s| s.kind == StatementKind::Assignment && s.target.as_deref() == Some("sLog27"))
    );
}
