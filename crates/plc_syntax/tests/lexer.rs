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

// PLC-76: IEC WSTRING double-quoted literals and `$` escapes must lex as a
// single string token with no invalid-token / unclosed diagnostics.
fn single_string_token(source: &str) -> String {
    let lexed = lex_source(source);
    assert!(
        lexed.diagnostics().is_empty(),
        "unexpected diagnostics for {source:?}: {:?}",
        lexed.diagnostics()
    );
    let strings: Vec<&plc_syntax::Token> = lexed
        .tokens()
        .iter()
        .filter(|token| token.kind == TokenKind::StringLiteral)
        .collect();
    assert_eq!(strings.len(), 1, "expected one string token in {source:?}");
    strings[0].text.clone()
}

#[test]
fn lexer_accepts_double_quoted_wstring_literals() {
    assert_eq!(single_string_token("\"hello\""), "\"hello\"");
    assert_eq!(single_string_token("\"\""), "\"\"");
}

#[test]
fn lexer_handles_dollar_escapes_in_strings() {
    // `$"` inside a WSTRING does not terminate it; `$0048` wide-char escape and
    // `$N` control escape are absorbed; `$'` works inside a single-quoted STRING.
    assert_eq!(single_string_token("\"a$\"b\""), "\"a$\"b\"");
    assert_eq!(single_string_token("\"$0048\""), "\"$0048\"");
    assert_eq!(single_string_token("\"line$N\""), "\"line$N\"");
    assert_eq!(single_string_token("'it$'s'"), "'it$'s'");
}

#[test]
fn lexer_accepts_at_prefixed_annotations() {
    // PLC-84: `@EXTERNAL`-style annotations lex as one identifier, not SYN0000,
    // and the following declaration still tokenizes cleanly.
    let lexed = lex_source("@EXTERNAL FUNCTION F : INT\nEND_FUNCTION\n");
    assert!(
        lexed.diagnostics().is_empty(),
        "unexpected diagnostics: {:?}",
        lexed.diagnostics()
    );
    assert!(
        lexed
            .tokens()
            .iter()
            .any(|token| token.kind == TokenKind::Identifier && token.text == "@EXTERNAL")
    );

    // A bare `@` (no following identifier) remains an invalid token.
    assert_eq!(lex_source("@ ").diagnostics().len(), 1);
}

#[test]
fn lexer_accepts_siemens_scl_local_refs() {
    // PLC-75: Siemens SCL hash-prefixed locals (`#Enable`) lex as one identifier
    // token, not SYN0000. A stray `#` not followed by a name still errors.
    let lexed = lex_source("#Enable := #Counter;");
    assert!(
        lexed.diagnostics().is_empty(),
        "unexpected diagnostics: {:?}",
        lexed.diagnostics()
    );
    let names: Vec<&str> = lexed
        .tokens()
        .iter()
        .filter(|token| token.kind == TokenKind::Identifier)
        .map(|token| token.text.as_str())
        .collect();
    assert!(names.contains(&"#Enable"));
    assert!(names.contains(&"#Counter"));

    // A bare `#` (no following identifier) remains an invalid token.
    assert_eq!(lex_source("# ").diagnostics().len(), 1);
}

#[test]
fn lexer_consumes_brace_pragmas_as_trivia() {
    // PLC-77: vendor pragmas / brace metadata are consumed as trivia and must
    // not cascade as SYN0000.
    let lexed = lex_source("{attribute 'hide'}\nFUNCTION_BLOCK FB\nEND_FUNCTION_BLOCK\n");
    assert!(
        lexed.diagnostics().is_empty(),
        "unexpected diagnostics: {:?}",
        lexed.diagnostics()
    );
    assert!(
        lexed
            .tokens()
            .iter()
            .any(|token| token.kind == TokenKind::Comment && token.text == "{attribute 'hide'}")
    );
    assert!(
        !lexed
            .tokens()
            .iter()
            .any(|token| token.kind == TokenKind::Invalid)
    );
}

#[test]
fn lexer_accepts_caret_dereference() {
    // PLC-74: `^` (pointer dereference) lexes as an operator, not SYN0000.
    let lexed = lex_source("ptr^.field := THIS^;");
    assert!(
        lexed.diagnostics().is_empty(),
        "unexpected diagnostics: {:?}",
        lexed.diagnostics()
    );
    assert_eq!(
        lexed
            .tokens()
            .iter()
            .filter(|token| token.kind == TokenKind::Operator && token.text == "^")
            .count(),
        2
    );
}

#[test]
fn lexer_reports_unclosed_double_quoted_string() {
    let lexed = lex_source("PROGRAM Main\n\"abc\nEND_PROGRAM\n");
    assert!(
        lexed
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == "SYN0001")
    );
}

// PLC-63: IEC located variables / direct addresses (`%IX0.0`, `%MW10`) must
// lex as a single token with no SYN0000.
fn single_address(source: &str) -> String {
    let lexed = lex_source(source);
    assert!(
        lexed.diagnostics().is_empty(),
        "unexpected diagnostics for {source:?}: {:?}",
        lexed.diagnostics()
    );
    let identifiers: Vec<&plc_syntax::Token> = lexed
        .tokens()
        .iter()
        .filter(|token| token.kind == TokenKind::Identifier)
        .collect();
    assert_eq!(
        identifiers.len(),
        1,
        "expected one address token in {source:?}"
    );
    identifiers[0].text.clone()
}

#[test]
fn lexer_accepts_located_variable_addresses() {
    assert_eq!(single_address("%IX7.8"), "%IX7.8");
    assert_eq!(single_address("%QX7.7"), "%QX7.7");
    assert_eq!(single_address("%IB4.8"), "%IB4.8");
    assert_eq!(single_address("%MW10"), "%MW10");
    assert_eq!(single_address("%I0.0"), "%I0.0");
    assert_eq!(single_address("%B6"), "%B6");
}

fn significant(source: &str) -> Vec<(TokenKind, String)> {
    let lexed = lex_source(source);
    assert!(
        lexed.diagnostics().is_empty(),
        "unexpected diagnostics for {source:?}: {:?}",
        lexed.diagnostics()
    );
    lexed
        .tokens()
        .iter()
        .filter(|token| !token.is_trivia())
        .map(|token| (token.kind, token.text.clone()))
        .collect()
}

#[test]
fn lexer_lexes_power_operator_as_single_token() {
    // PLC-86: `**` (exponentiation) must be one operator token, not two `*`.
    assert_eq!(
        significant("2.0 ** 10.0"),
        vec![
            (TokenKind::NumberLiteral, "2.0".to_owned()),
            (TokenKind::Operator, "**".to_owned()),
            (TokenKind::NumberLiteral, "10.0".to_owned()),
        ]
    );
    assert_eq!(
        significant("a ** b"),
        vec![
            (TokenKind::Identifier, "a".to_owned()),
            (TokenKind::Operator, "**".to_owned()),
            (TokenKind::Identifier, "b".to_owned()),
        ]
    );
}

#[test]
fn lexer_splits_range_after_bare_decimal() {
    // PLC-86: `1..10` must split into `1` `..` `10`, not glue as one literal.
    assert_eq!(
        significant("1..10"),
        vec![
            (TokenKind::NumberLiteral, "1".to_owned()),
            (TokenKind::Operator, "..".to_owned()),
            (TokenKind::NumberLiteral, "10".to_owned()),
        ]
    );
    assert_eq!(
        significant("ARRAY[0..255]"),
        vec![
            (TokenKind::Keyword, "ARRAY".to_owned()),
            (TokenKind::Operator, "[".to_owned()),
            (TokenKind::NumberLiteral, "0".to_owned()),
            (TokenKind::Operator, "..".to_owned()),
            (TokenKind::NumberLiteral, "255".to_owned()),
            (TokenKind::Operator, "]".to_owned()),
        ]
    );
}

#[test]
fn lexer_lexes_signed_exponent_real_literals() {
    // PLC-86: a signed decimal exponent stays part of the REAL literal.
    assert_eq!(single_literal("1.0e-3"), "1.0e-3");
    assert_eq!(single_literal("1.0E+3"), "1.0E+3");
    assert_eq!(single_literal("1.5E3"), "1.5E3");
    assert_eq!(single_literal("2.5e10"), "2.5e10");
}

#[test]
fn lexer_does_not_glue_member_access_after_decimal_point() {
    // PLC-86: `5.x` is `5` `.` `x` (a malformed/member form), not one literal.
    assert_eq!(
        significant("5.x"),
        vec![
            (TokenKind::NumberLiteral, "5".to_owned()),
            (TokenKind::Operator, ".".to_owned()),
            (TokenKind::Identifier, "x".to_owned()),
        ]
    );
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
