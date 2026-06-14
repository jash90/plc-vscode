use plc_syntax::{PouKind, StatementKind, VarBlockKind, parse_source};

#[test]
fn parser_recognizes_core_pou_program() {
    let parsed = parse_source("PROGRAM Main\nVAR\nEND_VAR\nEND_PROGRAM\n");

    assert!(parsed.diagnostics().is_empty());
    assert_eq!(parsed.units().len(), 1);
    assert_eq!(parsed.units()[0].kind, PouKind::Program);
    assert_eq!(parsed.units()[0].name.as_deref(), Some("Main"));
}

#[test]
fn parser_recognizes_mvp_declarations_and_statements() {
    let parsed = parse_source(
        "PROGRAM Main\nVAR_INPUT\n    Start : BOOL;\nEND_VAR\nVAR\n    Count : INT := 1;\nEND_VAR\nStart := TRUE;\nIF Start THEN\n    RETURN;\nEND_IF\nEND_PROGRAM\n",
    );

    assert!(parsed.diagnostics().is_empty());
    let unit = &parsed.units()[0];
    assert_eq!(unit.declaration_blocks.len(), 2);
    assert_eq!(unit.declaration_blocks[0].kind, VarBlockKind::Input);
    assert_eq!(unit.declaration_blocks[0].declarations[0].name, "Start");
    assert_eq!(unit.declaration_blocks[0].declarations[0].type_name, "BOOL");
    assert_eq!(unit.declaration_blocks[1].kind, VarBlockKind::Var);
    assert_eq!(
        unit.declaration_blocks[1].declarations[0]
            .initializer
            .as_deref(),
        Some("1")
    );
    assert!(
        unit.statements
            .iter()
            .any(|statement| statement.kind == StatementKind::Assignment)
    );
    assert!(
        unit.statements
            .iter()
            .any(|statement| statement.kind == StatementKind::If)
    );
    assert!(
        unit.statements
            .iter()
            .any(|statement| statement.kind == StatementKind::Return)
    );
}

#[test]
fn parser_reports_missing_program_terminator_with_stable_code() {
    let parsed = parse_source("PROGRAM Main\nVAR\nEND_VAR\n");

    assert_eq!(parsed.diagnostics().len(), 1);
    assert_eq!(parsed.diagnostics()[0].code, "PLC0002");
    assert!(parsed.diagnostics()[0].message.contains("END_PROGRAM"));
}

#[test]
fn parser_recovers_and_finds_following_pous() {
    let parsed =
        parse_source("PROGRAM Broken\nVAR\nEND_VAR\nFUNCTION_BLOCK Motor\nEND_FUNCTION_BLOCK\n");

    assert_eq!(parsed.diagnostics().len(), 1);
    assert_eq!(parsed.units().len(), 2);
    assert_eq!(parsed.units()[0].kind, PouKind::Program);
    assert_eq!(parsed.units()[1].kind, PouKind::FunctionBlock);
    assert_eq!(parsed.units()[1].name.as_deref(), Some("Motor"));
}

#[test]
fn parser_accepts_wstring_declarations() {
    // PLC-76: a WSTRING declaration with a double-quoted initializer and a `$N`
    // escape must parse without invalid-token/unclosed diagnostics.
    let parsed =
        parse_source("PROGRAM Main\nVAR\n    msg : WSTRING := \"hi$N\";\nEND_VAR\nEND_PROGRAM\n");

    assert!(
        !parsed
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == "SYN0000" || diagnostic.code == "SYN0001"),
        "unexpected string diagnostics: {:?}",
        parsed.diagnostics()
    );

    let declarations = &parsed.units()[0].declaration_blocks[0].declarations;
    assert!(
        declarations
            .iter()
            .any(|declaration| declaration.name == "msg" && declaration.type_name == "WSTRING")
    );
}

#[test]
fn parser_accepts_located_variable_declarations() {
    // PLC-63: `AT %IX0.0` located declarations must parse without SYN0000 and
    // still expose the variable name and type.
    let parsed = parse_source(
        "PROGRAM Main\nVAR\n    binvar AT %IX7.8 : BOOL;\n    inbyte AT %IB4.8 : BYTE;\nEND_VAR\nEND_PROGRAM\n",
    );

    assert!(
        !parsed
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == "SYN0000"),
        "unexpected invalid-token diagnostics: {:?}",
        parsed.diagnostics()
    );

    let declarations = &parsed.units()[0].declaration_blocks[0].declarations;
    assert!(
        declarations
            .iter()
            .any(|declaration| declaration.name == "binvar" && declaration.type_name == "BOOL")
    );
    assert!(
        declarations
            .iter()
            .any(|declaration| declaration.name == "inbyte" && declaration.type_name == "BYTE")
    );
}

#[test]
fn parser_accepts_typed_literals_without_invalid_token_diagnostics() {
    // PLC-62: duration literal in an assignment and BYTE# ranges in a CASE must
    // not produce SYN0000 invalid-token diagnostics for `#`.
    let parsed = parse_source(
        "PROGRAM Main\nVAR\n    Delay : TIME;\nEND_VAR\nDelay := T#20ms;\nCASE Code OF\n    BYTE#9..BYTE#10: Delay := T#0ms;\nEND_CASE\nEND_PROGRAM\n",
    );

    assert!(
        !parsed
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == "SYN0000"),
        "unexpected invalid-token diagnostics: {:?}",
        parsed.diagnostics()
    );
}
