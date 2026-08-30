//! Proves the language-plugin registry + canonical-IR conversion: a 2nd language
//! (IL) registers behind the registry and ST <-> IL converts through the IR hub.
//! Run with `cargo test -p plc_lang --features "st il"` (default features).

use plc_api::{DiagnosticSeverity, SourceDocument};
use plc_hir::lower_source;
use plc_lang::{ConversionError, LanguageRegistry};

const SRC: &str =
    "PROGRAM Main\nVAR\n    Count : INT;\nEND_VAR\nCount := Count + 1;\nEND_PROGRAM\n";

#[test]
fn registers_second_language_and_selects_by_id_and_extension() {
    let registry = LanguageRegistry::with_builtins();
    assert!(registry.frontend_by_id("st").is_some());
    assert!(registry.frontend_by_id("il").is_some(), "IL is registered");
    assert_eq!(
        registry.frontend_for_uri("file:///x.il").unwrap().id(),
        "il"
    );
    assert_eq!(
        registry.frontend_for_uri("file:///x.st").unwrap().id(),
        "st"
    );
    assert!(registry.ids().contains(&"il"));
}

#[test]
fn converts_st_to_il_through_the_ir_hub() {
    let registry = LanguageRegistry::with_builtins();
    let document = SourceDocument::new("file:///main.st", 0, SRC);

    let out = registry.convert("st", "il", &document);
    assert!(out.error.is_none(), "conversion succeeds: {:?}", out.error);
    assert!(
        out.diagnostics
            .iter()
            .all(|d| d.severity != DiagnosticSeverity::Error)
    );

    // The IL mirrors the pre-existing bytecode golden (LOAD_VAR/ADD/STORE_VAR).
    let lines: Vec<&str> = out
        .text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    assert!(lines.contains(&"LD Count"), "IL was:\n{}", out.text);
    assert!(lines.contains(&"ADD 1"), "IL was:\n{}", out.text);
    assert!(lines.contains(&"ST Count"), "IL was:\n{}", out.text);
}

#[test]
fn st_il_round_trip_preserves_ir_on_supported_subset() {
    let registry = LanguageRegistry::with_builtins();

    let ir0 = lower_source(SRC);
    let st_doc = SourceDocument::new("file:///main.st", 0, SRC);

    let il = registry.convert("st", "il", &st_doc).text; // ST -> IL
    let il_doc = SourceDocument::new("file:///main.il", 0, il);
    let back = registry.convert("il", "st", &il_doc).text; // IL -> ST

    let ir1 = lower_source(&back); // re-lower the round-tripped ST
    assert_eq!(
        ir0.programs[0].body, ir1.programs[0].body,
        "ST->IL->ST preserved the IR body"
    );
}

#[test]
fn unknown_target_errors_loudly_without_panicking() {
    let registry = LanguageRegistry::with_builtins();
    let document = SourceDocument::new("file:///main.st", 0, SRC);
    let out = registry.convert("st", "fbd", &document); // FBD not registered
    assert_eq!(
        out.error,
        Some(ConversionError::UnknownTarget("fbd".to_owned()))
    );
    assert!(out.text.is_empty());
}

#[test]
fn source_with_errors_does_not_convert() {
    let registry = LanguageRegistry::with_builtins();
    // Missing END_PROGRAM -> ST analyze reports an error -> no render attempted.
    let document = SourceDocument::new(
        "file:///bad.st",
        0,
        "PROGRAM Bad\nVAR\n    x : INT;\nEND_VAR",
    );
    let out = registry.convert("st", "il", &document);
    assert_eq!(out.error, Some(ConversionError::SourceHasErrors));
}
