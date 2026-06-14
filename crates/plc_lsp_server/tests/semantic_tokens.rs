use plc_lsp_server::{semantic_tokens_for_text, semantic_tokens_legend, server_capabilities};
use tower_lsp::lsp_types::{SemanticTokenType, SemanticTokensServerCapabilities};

// Semantic tokens protocol tests (PLC-59): assert the advertised provider, the
// stable legend, and the delta-encoded token output for representative source.

const SOURCE: &str = "PROGRAM Main\nVAR\n    Speed : INT;\nEND_VAR\nEND_PROGRAM\n";

#[test]
fn lsp_server_advertises_semantic_tokens_provider() {
    match server_capabilities().semantic_tokens_provider {
        Some(SemanticTokensServerCapabilities::SemanticTokensOptions(options)) => {
            assert!(
                options
                    .legend
                    .token_types
                    .contains(&SemanticTokenType::KEYWORD)
            );
        }
        other => panic!("unexpected semantic tokens provider: {other:?}"),
    }
}

#[test]
fn semantic_tokens_legend_lists_required_token_types() {
    let legend = semantic_tokens_legend();
    for token_type in [
        SemanticTokenType::KEYWORD,
        SemanticTokenType::TYPE,
        SemanticTokenType::VARIABLE,
        SemanticTokenType::FUNCTION,
        SemanticTokenType::CLASS,
    ] {
        assert!(
            legend.token_types.contains(&token_type),
            "legend missing {token_type:?}"
        );
    }
}

#[test]
fn semantic_tokens_encode_first_token_as_program_keyword() {
    let tokens = semantic_tokens_for_text("file:///main.st", 1, SOURCE);

    let first = tokens.data.first().expect("at least one semantic token");
    // `PROGRAM` at line 0, char 0, length 7, KEYWORD (legend index 0).
    assert_eq!(first.delta_line, 0);
    assert_eq!(first.delta_start, 0);
    assert_eq!(first.length, 7);
    assert_eq!(first.token_type, 0);
    assert_eq!(first.token_modifiers_bitset, 0);
}

#[test]
fn semantic_tokens_include_elementary_type() {
    let tokens = semantic_tokens_for_text("file:///main.st", 1, SOURCE);

    // TYPE is legend index 1; `INT` must be classified as a type.
    assert!(tokens.data.iter().any(|token| token.token_type == 1));
}
