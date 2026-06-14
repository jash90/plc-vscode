use plc_lsp_server::{definition_for_text, references_for_text, server_capabilities};
use tower_lsp::lsp_types::{OneOf, Position};

const SOURCE: &str =
    "PROGRAM Main\nVAR\n    Enabled : BOOL;\nEND_VAR\nEnabled := TRUE;\nEND_PROGRAM\n";

#[test]
fn lsp_server_advertises_navigation_support() {
    let capabilities = server_capabilities();
    assert_eq!(capabilities.definition_provider, Some(OneOf::Left(true)));
    assert_eq!(capabilities.references_provider, Some(OneOf::Left(true)));
}

#[test]
fn lsp_server_maps_definition_to_declaration() {
    let definition = definition_for_text(
        "file:///main.st",
        1,
        SOURCE,
        Position {
            line: 4,
            character: 2,
        },
    )
    .expect("definition location");
    assert_eq!(definition.range.start.line, 2);
}

#[test]
fn lsp_server_maps_references_with_declaration() {
    let references = references_for_text(
        "file:///main.st",
        1,
        SOURCE,
        Position {
            line: 4,
            character: 2,
        },
        true,
    );
    assert!(references.len() >= 2);
}
