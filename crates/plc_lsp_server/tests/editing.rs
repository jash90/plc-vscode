use plc_lsp_server::{formatting_edits_for_text, server_capabilities};
use tower_lsp::lsp_types::{CodeActionProviderCapability, OneOf};

#[test]
fn lsp_server_advertises_formatting_and_code_action_support() {
    let capabilities = server_capabilities();
    assert_eq!(
        capabilities.document_formatting_provider,
        Some(OneOf::Left(true))
    );
    assert_eq!(
        capabilities.document_range_formatting_provider,
        Some(OneOf::Left(true))
    );
    assert!(matches!(
        capabilities.code_action_provider,
        Some(CodeActionProviderCapability::Simple(true))
    ));
}

#[test]
fn lsp_server_formats_keyword_casing() {
    let edits = formatting_edits_for_text(
        "file:///main.st",
        1,
        "program Main\nvar\nEnabled : BOOL;\nend_var\nend_program\n",
    );
    assert_eq!(edits.len(), 1);
    assert!(edits[0].new_text.starts_with("PROGRAM Main"));
    assert!(edits[0].new_text.contains("END_PROGRAM"));
}
