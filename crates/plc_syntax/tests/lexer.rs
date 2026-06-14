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
