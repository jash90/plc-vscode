use plc_syntax::{PouKind, Statement, StatementKind, VarBlockKind, parse_source};

#[test]
fn parser_does_not_treat_member_access_targets_as_simple_assignments() {
    // PLC-79: qualified targets `obj.field := x` / `arr[i].field := x` must not be
    // captured as a simple assignment to the trailing member name, which the
    // analyzer cannot resolve and would flag as a spurious SEM0001.
    let parsed = parse_source(concat!(
        "PROGRAM Main\n",
        "VAR\n    obj : T;\nEND_VAR\n",
        "obj.field := 1;\n",
        "arr[2].state := 3;\n",
        "END_PROGRAM\n",
    ));

    let targets: Vec<&str> = parsed.units()[0]
        .statements
        .iter()
        .filter(|statement| statement.kind == StatementKind::Assignment)
        .filter_map(|statement| statement.target.as_deref())
        .collect();
    assert!(!targets.contains(&"field"), "got targets: {targets:?}");
    assert!(!targets.contains(&"state"), "got targets: {targets:?}");
}

#[test]
fn parser_does_not_treat_named_call_arguments_as_assignments() {
    // PLC-79: named call arguments `f(IN := x, L := y)` must NOT be parsed as
    // assignment statements. Previously every named argument became a spurious
    // assignment whose parameter name flooded SEM0001 (unresolved symbol).
    let parsed = parse_source(concat!(
        "PROGRAM Main\n",
        "VAR\n    s : STRING;\n    r : STRING;\nEND_VAR\n",
        "r := RIGHT(IN := s, L := 3);\n",
        "END_PROGRAM\n",
    ));

    let assignments: Vec<&Statement> = parsed.units()[0]
        .statements
        .iter()
        .filter(|statement| statement.kind == StatementKind::Assignment)
        .collect();
    assert_eq!(assignments.len(), 1, "got: {assignments:?}");
    assert_eq!(assignments[0].target.as_deref(), Some("r"));
}

#[test]
fn parser_captures_sized_string_initializer() {
    // PLC-87: a sized-type declaration must keep both its `[n]` clause and its
    // initializer, which the old code dropped because the `:=` check landed on
    // `[` and skipped to `;`.
    let parsed = parse_source(concat!(
        "PROGRAM Main\n",
        "VAR\n",
        "    s : STRING[20] := 'Jan';\n",
        "    log : STRING[80];\n",
        "END_VAR\n",
        "END_PROGRAM\n",
    ));

    let declarations = &parsed.units()[0].declaration_blocks[0].declarations;
    let s = declarations
        .iter()
        .find(|declaration| declaration.name == "s")
        .expect("declaration `s`");
    assert_eq!(s.type_name, "STRING");
    assert_eq!(s.type_size.as_deref(), Some("[20]"));
    assert_eq!(s.initializer.as_deref(), Some("'Jan'"));

    let log = declarations
        .iter()
        .find(|declaration| declaration.name == "log")
        .expect("declaration `log`");
    assert_eq!(log.type_name, "STRING");
    assert_eq!(log.type_size.as_deref(), Some("[80]"));
    assert_eq!(log.initializer, None);
}

#[test]
fn parser_captures_array_initializer_after_of_clause() {
    // PLC-87: `ARRAY[1..3] OF INT := [...]` must record the dimension clause and
    // still capture the initializer past the `OF <element-type>` clause.
    let parsed = parse_source(concat!(
        "PROGRAM Main\n",
        "VAR\n",
        "    a : ARRAY[1..3] OF INT := [1, 2, 3];\n",
        "END_VAR\n",
        "END_PROGRAM\n",
    ));

    let a = &parsed.units()[0].declaration_blocks[0].declarations[0];
    assert_eq!(a.name, "a");
    assert_eq!(a.type_name, "ARRAY");
    assert_eq!(a.type_size.as_deref(), Some("[1..3]"));
    assert!(
        a.initializer
            .as_deref()
            .is_some_and(|init| init.contains('1') && init.contains('2') && init.contains('3')),
        "got initializer: {:?}",
        a.initializer
    );
}

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
fn parser_recovers_codesys_interface_forward_declarations() {
    // PLC-78: `FUNCTION Name;` forward declarations in a CODESYS INTERFACE block
    // have no body/terminator and must not produce a SYN0002 missing-END_FUNCTION;
    // the real definition inside IMPLEMENTATION still parses as a unit.
    let parsed = parse_source(concat!(
        "UNIT UTIL;\n",
        "INTERFACE\n",
        "  USES OTHER;\n",
        "  FUNCTION SortArray;\n",
        "  FUNCTION ReverseArray;\n",
        "END_INTERFACE\n",
        "IMPLEMENTATION\n",
        "  FUNCTION SortArray\n",
        "  VAR_INPUT\n    a : INT;\n  END_VAR\n",
        "  END_FUNCTION\n",
        "END_IMPLEMENTATION\n",
    ));

    assert!(
        !parsed
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == "SYN0002"),
        "unexpected missing-terminator diagnostics: {:?}",
        parsed.diagnostics()
    );
    assert!(
        parsed
            .units()
            .iter()
            .any(|unit| unit.name.as_deref() == Some("SortArray"))
    );
}

#[test]
fn parser_recovers_configuration_blocks_without_spurious_terminator() {
    // PLC-73: an OpenPLC CONFIGURATION/RESOURCE/TASK wrapper after a complete
    // PROGRAM must not be parsed as a POU (the inner `PROGRAM instance … WITH`
    // mapping previously triggered a spurious PLC0002 missing END_PROGRAM).
    let parsed = parse_source(concat!(
        "PROGRAM Main\n",
        "VAR\n    x : BOOL;\nEND_VAR\n",
        "x := TRUE;\n",
        "END_PROGRAM\n",
        "\n",
        "CONFIGURATION Config0\n",
        "  RESOURCE Res0 ON PLC\n",
        "    TASK task0(INTERVAL := T#20ms,PRIORITY := 0);\n",
        "    PROGRAM instance0 WITH task0 : Main;\n",
        "  END_RESOURCE\n",
        "END_CONFIGURATION\n",
    ));

    assert!(
        !parsed
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == "PLC0002"),
        "unexpected missing-terminator diagnostics: {:?}",
        parsed.diagnostics()
    );
    assert_eq!(parsed.units().len(), 1);
    assert_eq!(parsed.units()[0].name.as_deref(), Some("Main"));
}

#[test]
fn parser_ignores_brace_pragmas() {
    // PLC-77: a leading `{attribute …}` pragma must not break POU parsing.
    let parsed = parse_source("{attribute 'hide'}\nFUNCTION_BLOCK Motor\nEND_FUNCTION_BLOCK\n");

    assert!(
        !parsed
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == "SYN0000"),
        "unexpected invalid-token diagnostics: {:?}",
        parsed.diagnostics()
    );
    assert_eq!(parsed.units().len(), 1);
    assert_eq!(parsed.units()[0].name.as_deref(), Some("Motor"));
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
