use plc_lsp_server::{completion_items_for_text, hover_for_text, server_capabilities};
use tower_lsp::lsp_types::{CompletionItemKind, HoverContents, HoverProviderCapability, Position};

const SOURCE: &str =
    "PROGRAM Main\nVAR\n    Enabled : BOOL;\nEND_VAR\nEnabled := TRUE;\nEND_PROGRAM\n";

#[test]
fn lsp_server_advertises_completion_and_hover_support() {
    let capabilities = server_capabilities();

    assert!(capabilities.completion_provider.is_some());
    assert_eq!(
        capabilities.hover_provider,
        Some(HoverProviderCapability::Simple(true))
    );
}

#[test]
fn lsp_server_maps_completion_items() {
    let completions = completion_items_for_text("file:///main.st", 1, SOURCE);

    assert!(completions.iter().any(|item| {
        item.label == "Enabled"
            && item.kind == Some(CompletionItemKind::VARIABLE)
            && item.detail.as_deref() == Some("BOOL")
    }));
    assert!(
        completions.iter().any(|item| {
            item.label == "PROGRAM" && item.kind == Some(CompletionItemKind::KEYWORD)
        })
    );
}

#[test]
fn lsp_server_maps_hover_payloads() {
    let hover = hover_for_text(
        "file:///main.st",
        1,
        SOURCE,
        Position {
            line: 2,
            character: 5,
        },
    )
    .expect("variable hover");

    match hover.contents {
        HoverContents::Markup(markup) => assert_eq!(markup.value, "Enabled: BOOL"),
        other => panic!("unexpected hover contents: {other:?}"),
    }
}
