use plc_syntax::{TokenKind, lex_source};

#[test]
fn lexer_preserves_trivia_and_source_ranges() {
    let lexed = lex_source("PROGRAM Main\n  (* comment *)\nEND_PROGRAM\n");

    assert!(lexed.diagnostics().is_empty());
    assert!(
        lexed
            .tokens()
            .iter()
            .any(|token| token.kind == TokenKind::Whitespace)
    );
    assert!(
        lexed
            .tokens()
            .iter()
            .any(|token| token.kind == TokenKind::Newline)
    );
    assert!(
        lexed
            .tokens()
            .iter()
            .any(|token| token.kind == TokenKind::Comment)
    );

    let program = lexed
        .tokens()
        .iter()
        .find(|token| token.text.eq_ignore_ascii_case("PROGRAM"))
        .expect("PROGRAM token should exist");
    assert_eq!(program.range.start, 0);
    assert_eq!(program.range.end, "PROGRAM".len());
}

#[test]
fn lexer_reports_recoverable_invalid_tokens() {
    let lexed = lex_source("PROGRAM Main\n@\nEND_PROGRAM\n");

    assert_eq!(lexed.diagnostics().len(), 1);
    assert_eq!(lexed.diagnostics()[0].code, "SYN0000");
    assert!(
        lexed
            .tokens()
            .iter()
            .any(|token| token.kind == TokenKind::Invalid)
    );
}

#[test]
fn lexer_reports_unclosed_nested_block_comment() {
    let lexed = lex_source("PROGRAM Main\n(* outer (* inner *)\nEND_PROGRAM\n");

    assert_eq!(lexed.diagnostics().len(), 1);
    assert_eq!(lexed.diagnostics()[0].code, "PLC0001");
    assert!(
        lexed.diagnostics()[0]
            .message
            .contains("Unclosed block comment")
    );
}

// PLC-62: IEC typed/base/duration literals use `#` (e.g. `16#FF`, `T#20ms`,
// `BYTE#9`). They must lex as a single literal token with no SYN0000.

fn single_literal(source: &str) -> String {
    let lexed = lex_source(source);
    assert!(
        lexed.diagnostics().is_empty(),
        "unexpected diagnostics for {source:?}: {:?}",
        lexed.diagnostics()
    );
    let literals: Vec<&plc_syntax::Token> = lexed
        .tokens()
        .iter()
        .filter(|token| token.kind == TokenKind::NumberLiteral)
        .collect();
    assert_eq!(
        literals.len(),
        1,
        "expected one literal token in {source:?}"
    );
    literals[0].text.clone()
}

#[test]
fn lexer_accepts_radix_integer_literals() {
    assert_eq!(single_literal("16#FF"), "16#FF");
    assert_eq!(single_literal("2#1010_0110"), "2#1010_0110");
    assert_eq!(single_literal("8#17"), "8#17");
}

#[test]
fn lexer_accepts_typed_and_duration_literals() {
    assert_eq!(single_literal("T#20ms"), "T#20ms");
    assert_eq!(single_literal("T#1.5s"), "T#1.5s");
    assert_eq!(single_literal("T#-5s"), "T#-5s");
    assert_eq!(single_literal("BOOL#1"), "BOOL#1");
    assert_eq!(single_literal("INT#16#FF"), "INT#16#FF");
    assert_eq!(single_literal("BYTE#9"), "BYTE#9");
}

#[test]
fn lexer_splits_typed_literal_case_ranges() {
    // `BYTE#9..BYTE#10` must keep the `..` range operator separate.
    let lexed = lex_source("BYTE#9..BYTE#10");
    assert!(lexed.diagnostics().is_empty());

    let significant: Vec<&plc_syntax::Token> = lexed
        .tokens()
        .iter()
        .filter(|token| !token.is_trivia())
        .collect();
    assert_eq!(significant.len(), 3);
    assert_eq!(significant[0].kind, TokenKind::NumberLiteral);
    assert_eq!(significant[0].text, "BYTE#9");
    assert_eq!(significant[1].kind, TokenKind::Operator);
    assert_eq!(significant[1].text, "..");
    assert_eq!(significant[2].kind, TokenKind::NumberLiteral);
    assert_eq!(significant[2].text, "BYTE#10");
}
