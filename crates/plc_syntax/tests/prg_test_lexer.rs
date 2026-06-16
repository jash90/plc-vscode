//! Lexer coverage derived from `PRG_Test_ST.st` — operator-keywords, operators,
//! literals, comments. Snippets are taken/adapted from the file.

use plc_syntax::{TokenKind, lex_source};

const FIXTURE: &str = include_str!("fixtures/prg_test_st.st");

/// Kind of the first token whose text equals `text`.
fn kind_of(src: &str, text: &str) -> TokenKind {
    lex_source(src)
        .tokens()
        .iter()
        .find(|token| token.text == text)
        .unwrap_or_else(|| panic!("token {text:?} not found in {src:?}"))
        .kind
}

fn significant(src: &str) -> Vec<(TokenKind, String)> {
    lex_source(src)
        .tokens()
        .iter()
        .filter(|token| !token.is_trivia())
        .map(|token| (token.kind, token.text.clone()))
        .collect()
}

#[test]
fn operator_keyword_and_is_keyword() {
    assert_eq!(kind_of("xWynik := xA AND xB;", "AND"), TokenKind::Keyword);
}

#[test]
fn operator_keyword_or_is_keyword() {
    assert_eq!(kind_of("xWynik := xA OR xB;", "OR"), TokenKind::Keyword);
}

#[test]
fn operator_keyword_xor_is_keyword() {
    assert_eq!(kind_of("xWynik := xA XOR xB;", "XOR"), TokenKind::Keyword);
}

#[test]
fn operator_keyword_not_is_keyword() {
    assert_eq!(kind_of("xWynik := NOT xA;", "NOT"), TokenKind::Keyword);
}

#[test]
fn operator_keyword_mod_is_keyword() {
    assert_eq!(kind_of("iWynik := 17 MOD 5;", "MOD"), TokenKind::Keyword);
    assert_eq!(
        kind_of("iWynik := 17 MOD 5;", "17"),
        TokenKind::NumberLiteral
    );
}

#[test]
fn shl_shr_ton_ctu_are_identifiers_not_keywords() {
    assert_eq!(
        kind_of("wWynik := SHL(wA, 4);", "SHL"),
        TokenKind::Identifier
    );
    assert_eq!(
        kind_of("wWynik := SHR(wB, 4);", "SHR"),
        TokenKind::Identifier
    );
    assert_eq!(kind_of("fbTON : TON;", "TON"), TokenKind::Identifier);
    assert_eq!(kind_of("fbCTU : CTU;", "CTU"), TokenKind::Identifier);
}

#[test]
fn comparison_operators_are_single_tokens() {
    assert_eq!(kind_of("xWynik := (iA < iB);", "<"), TokenKind::Operator);
    let toks = significant("WHILE iWynik <= 100 DO");
    assert!(
        toks.iter()
            .any(|(k, t)| *k == TokenKind::Operator && t == "<="),
        "<= must be one operator token: {toks:?}"
    );
    assert_eq!(
        kind_of("WHILE iWynik <= 100 DO", "WHILE"),
        TokenKind::Keyword
    );
}

#[test]
fn assignment_operator_is_single_token() {
    let toks = significant("fbCTU(CU := xTakt, RESET := FALSE, PV := 10);");
    let assigns = toks
        .iter()
        .filter(|(k, t)| *k == TokenKind::Operator && t == ":=")
        .count();
    assert_eq!(assigns, 3, "three := operators expected: {toks:?}");
    assert!(
        !toks
            .iter()
            .any(|(k, t)| *k == TokenKind::Operator && t == "=")
    );
}

#[test]
fn hex_word_initializer_is_one_number_literal() {
    let lexed = lex_source("wA : WORD := 16#000F;");
    let hex: Vec<_> = lexed
        .tokens()
        .iter()
        .filter(|t| t.kind == TokenKind::NumberLiteral)
        .collect();
    assert_eq!(hex.len(), 1);
    assert_eq!(hex[0].text, "16#000F");
    assert!(lexed.diagnostics().is_empty());
}

#[test]
fn duration_initializer_is_one_number_literal() {
    let lexed = lex_source("tA : TIME := T#1h30m;");
    let durations: Vec<_> = lexed
        .tokens()
        .iter()
        .filter(|t| t.kind == TokenKind::NumberLiteral)
        .collect();
    assert_eq!(durations.len(), 1);
    assert_eq!(durations[0].text, "T#1h30m");
    assert!(lexed.diagnostics().is_empty());
}

#[test]
fn string_literal_with_inner_operators_is_one_token() {
    let lexed = lex_source("sLog01 := CONCAT('2 + 3 = ', INT_TO_STRING(iWynik));");
    let strings: Vec<_> = lexed
        .tokens()
        .iter()
        .filter(|t| t.kind == TokenKind::StringLiteral)
        .collect();
    assert_eq!(strings.len(), 1);
    assert_eq!(strings[0].text, "'2 + 3 = '");
}

#[test]
fn member_access_dot_is_operator() {
    let toks = significant("sLog := TIME_TO_STRING(fbTON.ET);");
    assert!(toks.contains(&(TokenKind::Identifier, "fbTON".to_owned())));
    assert!(toks.contains(&(TokenKind::Operator, ".".to_owned())));
    assert!(toks.contains(&(TokenKind::Identifier, "ET".to_owned())));
}

#[test]
fn line_and_block_comments_are_trivia() {
    let lexed = lex_source("iWynik := iA + iB;   // oczekiwane: 5\n");
    let line_comment = lexed
        .tokens()
        .iter()
        .find(|t| t.kind == TokenKind::Comment)
        .expect("line comment token");
    assert!(line_comment.text.starts_with("//"));
    assert!(line_comment.is_trivia());

    let block = lex_source("(*───── 1) ARYTMETYKA ─────*)");
    let comments: Vec<_> = block
        .tokens()
        .iter()
        .filter(|t| t.kind == TokenKind::Comment)
        .collect();
    assert_eq!(comments.len(), 1, "box-drawing block comment is one token");
    assert!(block.diagnostics().is_empty());
    assert!(block.tokens().iter().all(|t| t.kind != TokenKind::Invalid));
}

#[test]
fn full_fixture_has_no_invalid_tokens() {
    let lexed = lex_source(FIXTURE);
    assert!(lexed.tokens().iter().all(|t| t.kind != TokenKind::Invalid));
}

#[test]
fn full_fixture_has_no_lexer_diagnostics() {
    assert!(lex_source(FIXTURE).diagnostics().is_empty());
}

#[test]
fn full_fixture_contains_all_operator_keywords() {
    let lexed = lex_source(FIXTURE);
    for keyword in ["AND", "OR", "XOR", "NOT", "MOD"] {
        assert!(
            lexed.tokens().iter().any(|t| t.keyword_eq(keyword)),
            "operator keyword {keyword} not classified in the fixture"
        );
    }
}
