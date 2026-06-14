use plc_syntax::{PouKind, parse_source};

#[test]
fn parser_recognizes_core_pou_program() {
    let parsed = parse_source("PROGRAM Main\nVAR\nEND_VAR\nEND_PROGRAM\n");

    assert!(parsed.diagnostics().is_empty());
    assert_eq!(parsed.units().len(), 1);
    assert_eq!(parsed.units()[0].kind, PouKind::Program);
    assert_eq!(parsed.units()[0].name.as_deref(), Some("Main"));
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
