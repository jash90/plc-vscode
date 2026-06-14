use plc_syntax::{PouKind, parse_source};

#[test]
fn malformed_fixture_keeps_following_pou_parseable() {
    let source = include_str!("../test_data/parser/malformed_missing_end_program.st");
    let parsed = parse_source(source);

    assert_eq!(parsed.diagnostics().len(), 1);
    assert_eq!(parsed.diagnostics()[0].code, "PLC0002");
    assert_eq!(parsed.units().len(), 2);
    assert_eq!(parsed.units()[1].kind, PouKind::FunctionBlock);
}
