//! LD → ST conversion through the IR hub: a `.ld` JSON program registers
//! behind the language registry and converts to Structured Text via HIR.

use plc_api::SourceDocument;
use plc_lang::LanguageRegistry;

/// A simple LD program: `(A AND NOT B) → C` as JSON.
const SIMPLE_LD: &str = r#"{
    "name": "Motor",
    "rungs": [
        {
            "branches": [
                {
                    "elements": [
                        { "name": "A", "negated": false },
                        { "name": "B", "negated": true }
                    ]
                }
            ],
            "outputs": [
                {
                    "kind": "coil",
                    "name": "C",
                    "variant": "normal"
                }
            ]
        }
    ]
}"#;

/// LD program with parallel branches: `(A OR B) → C`.
const OR_LD: &str = r#"{
    "name": "OrTest",
    "rungs": [
        {
            "branches": [
                {
                    "elements": [{ "name": "A", "negated": false }]
                },
                {
                    "elements": [{ "name": "B", "negated": false }]
                }
            ],
            "outputs": [
                {
                    "kind": "coil",
                    "name": "C",
                    "variant": "normal"
                }
            ]
        }
    ]
}"#;

#[test]
fn ld_registers_in_the_registry() {
    let registry = LanguageRegistry::with_builtins();
    assert!(registry.frontend_by_id("ld").is_some(), "LD is registered");
    assert!(registry.ids().contains(&"ld"));
    assert_eq!(
        registry.frontend_for_uri("file:///x.ld").unwrap().id(),
        "ld"
    );
}

#[test]
fn converts_ld_to_st_with_and_not() {
    let registry = LanguageRegistry::with_builtins();
    let document = SourceDocument::new("file:///motor.ld", 0, SIMPLE_LD);

    let out = registry.convert("ld", "st", &document);
    assert!(out.error.is_none(), "conversion succeeds: {:?}", out.error);

    // Should contain PROGRAM, VAR, END_PROGRAM, and the assignment.
    let st = &out.text;
    assert!(st.contains("PROGRAM"), "ST was:\n{st}");
    assert!(st.contains("END_PROGRAM"), "ST was:\n{st}");
    assert!(st.contains("VAR"), "ST was:\n{st}");
    // The assignment: C := A AND NOT B;
    assert!(st.contains("AND"), "ST was:\n{st}");
    assert!(st.contains("NOT"), "ST was:\n{st}");
    assert!(
        st.contains("C :="),
        "ST should assign to C, was:\n{st}"
    );
}

#[test]
fn converts_ld_to_st_with_or() {
    let registry = LanguageRegistry::with_builtins();
    let document = SourceDocument::new("file:///or.ld", 0, OR_LD);

    let out = registry.convert("ld", "st", &document);
    assert!(out.error.is_none(), "conversion succeeds: {:?}", out.error);
    assert!(out.text.contains("OR"), "ST was:\n{}", out.text);
}

#[test]
fn ld_to_st_produces_executable_program() {
    // The ST produced from LD should be valid enough that the ST frontend can
    // re-lower it. We verify by converting LD→ST, then checking the ST text
    // is well-formed (has PROGRAM/VAR/END_VAR/assignment/END_PROGRAM).
    let registry = LanguageRegistry::with_builtins();
    let document = SourceDocument::new("file:///motor.ld", 0, SIMPLE_LD);
    let st = registry.convert("ld", "st", &document).text;

    // Re-lower the ST through the ST frontend to verify it's valid.
    let st_doc = SourceDocument::new("file:///motor.st", 0, &st);
    let lowered = registry.frontend_by_id("st").unwrap().lower(&st_doc);
    assert!(
        lowered
            .diagnostics
            .iter()
            .all(|d| d.severity != plc_api::DiagnosticSeverity::Error),
        "ST produced from LD has errors: {:?}",
        lowered.diagnostics
    );
}

#[test]
fn invalid_ld_json_produces_error() {
    let registry = LanguageRegistry::with_builtins();
    let document = SourceDocument::new("file:///bad.ld", 0, "not valid json");
    let out = registry.convert("ld", "st", &document);
    // The LD frontend emits an Error diagnostic -> SourceHasErrors.
    assert!(
        out.error.is_some(),
        "invalid JSON should produce an error: {:?}",
        out.error
    );
}
