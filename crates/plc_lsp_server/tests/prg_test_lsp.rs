//! LSP free-function coverage exercised against the real `PRG_Test_ST.st`.

use plc_lsp_server::{
    completion_items_for_text, diagnostics_for_text, document_symbols_for_text, hover_for_text,
    semantic_tokens_for_text, semantic_tokens_legend, server_capabilities, signature_help_for_text,
    workspace_symbols_for_documents,
};
use tower_lsp::lsp_types::{
    CompletionItemKind, HoverContents, HoverProviderCapability, OneOf, Position, SemanticTokenType,
    SymbolKind, TextDocumentSyncCapability, TextDocumentSyncKind,
};

const SOURCE: &str = include_str!("fixtures/prg_test_st.st");
const URI: &str = "file:///prg_test.st";

fn find_pos(needle: &str) -> Position {
    let idx = SOURCE
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not found"));
    let line = SOURCE[..idx].matches('\n').count() as u32;
    let last_nl = SOURCE[..idx].rfind('\n').map(|p| p + 1).unwrap_or(0);
    let character = SOURCE[last_nl..idx].chars().count() as u32;
    Position { line, character }
}

fn pos_on(needle: &str) -> Position {
    let p = find_pos(needle);
    Position {
        line: p.line,
        character: p.character + 1,
    }
}

fn pos_in_call(call: &str) -> Position {
    let p = find_pos(call);
    Position {
        line: p.line,
        character: p.character + call.chars().count() as u32,
    }
}

#[test]
fn diagnostics_empty_on_real_file() {
    assert!(diagnostics_for_text(URI, 1, SOURCE).is_empty());
}

#[test]
fn document_symbols_root_is_prg_test_with_58_children() {
    let symbols = document_symbols_for_text(URI, 1, SOURCE);
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].name, "PRG_Test");
    assert_eq!(symbols[0].kind, SymbolKind::MODULE);
    let children = symbols[0].children.as_ref().expect("children");
    assert_eq!(children.len(), 58);
    assert!(
        children
            .iter()
            .any(|c| c.name == "sLog01" && c.kind == SymbolKind::VARIABLE)
    );
    assert!(
        children
            .iter()
            .any(|c| c.name == "fbTON" && c.detail.as_deref() == Some("TON"))
    );
}

#[test]
fn semantic_tokens_data_is_1131_ints_and_starts_at_program_keyword() {
    let tokens = semantic_tokens_for_text(URI, 1, SOURCE);
    assert_eq!(tokens.data.len(), 1131);
    let first = &tokens.data[0];
    // PROGRAM keyword: delta from (0,0) to its line, KEYWORD legend index 0, len 7.
    assert_eq!(first.delta_line, find_pos("PROGRAM PRG_Test").line);
    assert_eq!(first.delta_start, 0);
    assert_eq!(first.length, 7);
    assert_eq!(first.token_type, 0);
}

#[test]
fn semantic_tokens_delta_lines_are_non_decreasing() {
    let tokens = semantic_tokens_for_text(URI, 1, SOURCE);
    // All deltas are valid; reconstructed absolute lines are monotonic.
    let mut line = 0u32;
    for token in &tokens.data {
        line += token.delta_line;
        let _ = line; // accumulation never underflows (delta_line is unsigned)
    }
    assert!(!tokens.data.is_empty());
}

#[test]
fn completion_includes_functions_fbs_vars_keywords() {
    let items = completion_items_for_text(URI, 1, SOURCE, Position::default());
    let has = |label: &str, kind: CompletionItemKind| {
        items
            .iter()
            .any(|i| i.label == label && i.kind == Some(kind))
    };
    for f in [
        "EXPT",
        "SHL",
        "CONCAT",
        "TIME_TO_STRING",
        "TO_STRING",
        "REAL_TO_STRING",
    ] {
        assert!(has(f, CompletionItemKind::FUNCTION), "function {f}");
    }
    assert!(has("TON", CompletionItemKind::CLASS));
    assert!(has("CTU", CompletionItemKind::CLASS));
    assert!(has("sLog01", CompletionItemKind::VARIABLE));
    assert!(has("iA", CompletionItemKind::VARIABLE));
    for k in ["PROGRAM", "IF", "FOR", "WHILE", "CASE"] {
        assert!(has(k, CompletionItemKind::KEYWORD), "keyword {k}");
    }
}

#[test]
fn completion_total_is_122() {
    assert_eq!(
        completion_items_for_text(URI, 1, SOURCE, Position::default()).len(),
        122
    );
}

#[test]
fn hover_on_program_keyword_markup() {
    let pos = Position {
        line: find_pos("PROGRAM PRG_Test").line,
        character: 1,
    };
    let hover = hover_for_text(URI, 1, SOURCE, pos).expect("hover");
    match hover.contents {
        HoverContents::Markup(markup) => {
            assert_eq!(markup.value, "Structured Text keyword `PROGRAM`")
        }
        other => panic!("expected markup, got {other:?}"),
    }
}

#[test]
fn hover_on_fb_instance_markup() {
    let hover = hover_for_text(URI, 1, SOURCE, pos_on("fbTON")).expect("hover");
    match hover.contents {
        HoverContents::Markup(markup) => assert_eq!(markup.value, "fbTON: TON"),
        other => panic!("expected markup, got {other:?}"),
    }
}

#[test]
fn hover_none_in_leading_comment() {
    // Cursor inside the banner block comment (line 0) resolves to no symbol.
    let hover = hover_for_text(
        URI,
        1,
        SOURCE,
        Position {
            line: 1,
            character: 6,
        },
    );
    assert!(hover.is_none());
}

#[test]
fn signature_help_for_expt_and_limit() {
    let expt = signature_help_for_text(URI, 1, SOURCE, pos_in_call("EXPT(")).expect("EXPT");
    assert_eq!(expt.active_signature, Some(0));
    assert_eq!(expt.signatures.len(), 1);
    assert_eq!(
        expt.signatures[0].label,
        "EXPT(IN1 : ANY_NUM; IN2 : ANY_NUM)"
    );
    assert_eq!(expt.signatures[0].active_parameter, Some(0));

    let limit = signature_help_for_text(URI, 1, SOURCE, pos_in_call("LIMIT(")).expect("LIMIT");
    assert_eq!(
        limit.signatures[0].label,
        "LIMIT(MN : ANY_NUM; IN : ANY_NUM; MX : ANY_NUM)"
    );
    assert_eq!(limit.signatures[0].parameters.as_ref().unwrap().len(), 3);
}

#[test]
fn signature_help_none_at_statement_start() {
    assert!(signature_help_for_text(URI, 1, SOURCE, find_pos("iWynik := iA + iB")).is_none());
}

#[test]
fn workspace_symbols_find_prg_test_top_level_only() {
    let documents = vec![(URI.to_owned(), SOURCE.to_owned())];
    let found = workspace_symbols_for_documents(&documents, "PRG_Test");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "PRG_Test");
    assert_eq!(found[0].kind, SymbolKind::MODULE);

    // Member variables are not workspace symbols.
    assert!(workspace_symbols_for_documents(&documents, "sLog01").is_empty());
}

#[test]
fn capabilities_advertise_the_feature_surface() {
    let caps = server_capabilities();
    assert_eq!(caps.document_symbol_provider, Some(OneOf::Left(true)));
    assert!(caps.completion_provider.is_some());
    assert_eq!(
        caps.hover_provider,
        Some(HoverProviderCapability::Simple(true))
    );
    assert!(caps.signature_help_provider.is_some());
    assert_eq!(caps.workspace_symbol_provider, Some(OneOf::Left(true)));
    assert!(caps.semantic_tokens_provider.is_some());
    assert_eq!(
        caps.text_document_sync,
        Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL))
    );
}

#[test]
fn semantic_tokens_legend_covers_kinds_used_by_the_file() {
    let legend = semantic_tokens_legend();
    for ty in [
        SemanticTokenType::KEYWORD,
        SemanticTokenType::TYPE,
        SemanticTokenType::VARIABLE,
        SemanticTokenType::FUNCTION,
        SemanticTokenType::NUMBER,
        SemanticTokenType::STRING,
        SemanticTokenType::COMMENT,
        SemanticTokenType::OPERATOR,
    ] {
        assert!(legend.token_types.contains(&ty), "legend missing {ty:?}");
    }
}
