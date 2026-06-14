use plc_syntax::cst::{SyntaxKind, build_cst};

#[test]
fn rowan_cst_preserves_trivia_tokens_and_ranges() {
    let source = "PROGRAM Main\n  (* comment *)\nEND_PROGRAM\n";
    let cst = build_cst(source);
    let root = cst.root();

    assert_eq!(root.kind(), SyntaxKind::Root);
    assert_eq!(root.text().to_string(), source);

    let tokens: Vec<_> = cst.tokens().collect();
    assert!(
        tokens
            .iter()
            .any(|token| token.kind() == SyntaxKind::Whitespace)
    );
    assert!(
        tokens
            .iter()
            .any(|token| token.kind() == SyntaxKind::Comment)
    );
    assert!(
        tokens
            .iter()
            .any(|token| token.kind() == SyntaxKind::Newline)
    );

    let program = tokens
        .iter()
        .find(|token| token.text() == "PROGRAM")
        .expect("PROGRAM token should exist");
    let start: u32 = program.text_range().start().into();
    assert_eq!(start, 0);
}
