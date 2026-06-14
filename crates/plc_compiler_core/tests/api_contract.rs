use plc_compiler_core::{CompilerCore, DiagnosticSeverity, Position, SourceDocument, SymbolKind};

#[test]
fn compiler_core_formats_keyword_casing_and_indentation() {
    let core = CompilerCore;
    let document = SourceDocument::new(
        "file:///main.st",
        1,
        "program Main\nvar\nEnabled : BOOL;\nend_var\nend_program\n",
    );

    let edits = core.formatting(&document);
    assert_eq!(edits.len(), 1);
    assert_eq!(
        edits[0].new_text,
        "PROGRAM Main\n    VAR\n        Enabled : BOOL;\n    END_VAR\nEND_PROGRAM\n"
    );
}

#[test]
fn compiler_core_formatting_is_idempotent_for_clean_source() {
    let core = CompilerCore;
    let document = SourceDocument::new(
        "file:///main.st",
        1,
        "PROGRAM Main\n    VAR\n        Enabled : BOOL;\n    END_VAR\nEND_PROGRAM\n",
    );

    assert!(core.formatting(&document).is_empty());
}

#[test]
fn compiler_core_offers_quick_fix_for_missing_terminator() {
    let core = CompilerCore;
    let document = SourceDocument::new("file:///main.st", 1, "PROGRAM Main\nVAR\nEND_VAR\n");

    let actions = core.code_actions(&document);
    assert!(
        actions
            .iter()
            .any(|action| action.title.contains("END_PROGRAM") && !action.edits.is_empty())
    );
}

#[test]
fn compiler_core_resolves_definition_to_declaration() {
    let core = CompilerCore;
    let document = SourceDocument::new(
        "file:///main.st",
        1,
        "PROGRAM Main\nVAR\n    Enabled : BOOL;\nEND_VAR\nEnabled := TRUE;\nEND_PROGRAM\n",
    );

    // Position on the `Enabled` usage in the assignment (line 4).
    let definition = core
        .definition(
            &document,
            Position {
                line: 4,
                character: 2,
            },
        )
        .expect("definition for Enabled");

    assert_eq!(definition.uri, "file:///main.st");
    // Declaration is on line 2.
    assert_eq!(definition.range.start.line, 2);
}

#[test]
fn compiler_core_finds_references_including_declaration() {
    let core = CompilerCore;
    let document = SourceDocument::new(
        "file:///main.st",
        1,
        "PROGRAM Main\nVAR\n    Enabled : BOOL;\nEND_VAR\nEnabled := TRUE;\nEND_PROGRAM\n",
    );

    let references = core.references(
        &document,
        Position {
            line: 4,
            character: 2,
        },
        true,
    );

    // Declaration occurrence + assignment usage.
    assert!(references.len() >= 2);
    assert!(
        references
            .iter()
            .all(|location| location.uri == "file:///main.st")
    );
    assert!(
        references
            .iter()
            .any(|location| location.range.start.line == 2)
    );
    assert!(
        references
            .iter()
            .any(|location| location.range.start.line == 4)
    );
}

#[test]
fn compiler_core_returns_completion_candidates_for_symbols_and_keywords() {
    let core = CompilerCore;
    let document = SourceDocument::new(
        "file:///main.st",
        1,
        "PROGRAM Main\nVAR\n    Enabled : BOOL;\nEND_VAR\nEND_PROGRAM\n",
    );

    let completions = core.completions(&document, Position::default());

    assert!(
        completions
            .iter()
            .any(|item| item.label == "Enabled" && item.detail.as_deref() == Some("BOOL"))
    );
    assert!(
        completions
            .iter()
            .any(|item| item.label == "PROGRAM" && item.kind == SymbolKind::Keyword)
    );
}

const PROGRAM_WITH_FB_MEMBER: &str = concat!(
    "FUNCTION_BLOCK Counter\n",
    "VAR_INPUT\n",
    "    CU : BOOL;\n",
    "END_VAR\n",
    "VAR_OUTPUT\n",
    "    Q : BOOL;\n",
    "END_VAR\n",
    "END_FUNCTION_BLOCK\n",
    "PROGRAM Main\n",
    "VAR\n",
    "    inst : Counter;\n",
    "END_VAR\n",
    "inst.\n",
    "END_PROGRAM\n",
);

#[test]
fn compiler_core_completion_includes_standard_functions_and_blocks() {
    let core = CompilerCore;
    let document = SourceDocument::new(
        "file:///main.st",
        1,
        "PROGRAM Main\nVAR\nEND_VAR\nEND_PROGRAM\n",
    );

    let completions = core.completions(&document, Position::default());

    assert!(completions.iter().any(|item| item.label == "MIN"
        && item.kind == SymbolKind::Function
        && item.detail.as_deref() == Some("standard function")));
    assert!(completions.iter().any(|item| item.label == "TON"
        && item.kind == SymbolKind::FunctionBlock
        && item.detail.as_deref() == Some("standard function block")));
}

#[test]
fn compiler_core_completion_suggests_user_fb_members_on_member_access() {
    let core = CompilerCore;
    let document = SourceDocument::new("file:///main.st", 1, PROGRAM_WITH_FB_MEMBER);

    // Cursor right after `inst.` on line 12.
    let completions = core.completions(
        &document,
        Position {
            line: 12,
            character: 5,
        },
    );

    assert!(
        completions
            .iter()
            .any(|item| item.label == "CU" && item.detail.as_deref() == Some("member of BOOL"))
    );
    assert!(completions.iter().any(|item| item.label == "Q"));
    // Member-access context returns members only — no keywords or POUs.
    assert!(
        !completions
            .iter()
            .any(|item| item.kind == SymbolKind::Keyword)
    );
}

#[test]
fn compiler_core_completion_suggests_standard_fb_members_on_member_access() {
    let core = CompilerCore;
    let document = SourceDocument::new(
        "file:///main.st",
        1,
        "PROGRAM Main\nVAR\n    t : TON;\nEND_VAR\nt.\nEND_PROGRAM\n",
    );

    // Cursor right after `t.` on line 4.
    let completions = core.completions(
        &document,
        Position {
            line: 4,
            character: 2,
        },
    );

    let labels: Vec<&str> = completions.iter().map(|item| item.label.as_str()).collect();
    for member in ["IN", "PT", "Q", "ET"] {
        assert!(labels.contains(&member), "expected TON member {member}");
    }
}

#[test]
fn compiler_core_returns_hover_for_variable_and_keywords() {
    let core = CompilerCore;
    let document = SourceDocument::new(
        "file:///main.st",
        1,
        "PROGRAM Main\nVAR\n    Enabled : BOOL;\nEND_VAR\nEnabled := TRUE;\nEND_PROGRAM\n",
    );

    let variable_hover = core
        .hover(
            &document,
            Position {
                line: 2,
                character: 5,
            },
        )
        .expect("variable hover");
    assert_eq!(variable_hover.contents, "Enabled: BOOL");

    let keyword_hover = core
        .hover(
            &document,
            Position {
                line: 0,
                character: 1,
            },
        )
        .expect("keyword hover");
    assert!(keyword_hover.contents.contains("PROGRAM"));
}

#[test]
fn compiler_core_returns_hierarchical_document_symbols() {
    let core = CompilerCore;
    let document = SourceDocument::new(
        "file:///main.st",
        3,
        "PROGRAM Main\nVAR\n    Enabled : BOOL;\nEND_VAR\nEND_PROGRAM\n",
    );

    let symbols = core.document_symbols(&document);

    assert_eq!(symbols.uri(), "file:///main.st");
    assert_eq!(symbols.version(), 3);
    assert_eq!(symbols.symbols().len(), 1);
    assert_eq!(symbols.symbols()[0].name, "Main");
    assert_eq!(symbols.symbols()[0].children.len(), 1);
    assert_eq!(symbols.symbols()[0].children[0].name, "Enabled");
    assert_eq!(
        symbols.symbols()[0].children[0].detail.as_deref(),
        Some("BOOL")
    );
}

#[test]
fn compiler_core_uses_syntax_ranges_for_diagnostics() {
    let core = CompilerCore;
    let document = SourceDocument::new(
        "file:///main.st",
        1,
        "// banner\nPROGRAM Main\nVAR\nEND_VAR\n",
    );

    let analysis = core.analyze(&document);

    assert_eq!(analysis.diagnostics().len(), 1);
    assert_eq!(analysis.diagnostics()[0].code, "PLC0002");
    assert_eq!(analysis.diagnostics()[0].range.start.line, 1);
    assert_eq!(analysis.diagnostics()[0].range.start.character, 0);
}

#[test]
fn compiler_core_surfaces_semantic_diagnostics() {
    let core = CompilerCore;
    let document = SourceDocument::new(
        "file:///main.st",
        1,
        "PROGRAM Main\nVAR\n    Enabled : BOOL;\nEND_VAR\nEnabled := 'yes';\nEND_PROGRAM\n",
    );

    let analysis = core.analyze(&document);

    assert_eq!(analysis.diagnostics().len(), 1);
    assert_eq!(analysis.diagnostics()[0].code, "SEM0002");
    assert!(analysis.diagnostics()[0].message.contains("BOOL"));
}

#[test]
fn compiler_core_analyzes_text_and_returns_versioned_diagnostics() {
    let core = CompilerCore;
    let document = SourceDocument::new("file:///main.st", 7, "PROGRAM Main\nVAR\nEND_VAR\n");

    let analysis = core.analyze(&document);

    assert_eq!(analysis.uri(), "file:///main.st");
    assert_eq!(analysis.version(), 7);
    assert_eq!(analysis.diagnostics().len(), 1);
    assert_eq!(
        analysis.diagnostics()[0].severity,
        DiagnosticSeverity::Error
    );
    assert!(analysis.diagnostics()[0].message.contains("END_PROGRAM"));
}

#[test]
fn compiler_core_returns_no_diagnostics_for_minimal_program() {
    let core = CompilerCore;
    let document = SourceDocument::new(
        "file:///main.st",
        1,
        "PROGRAM Main\nVAR\nEND_VAR\nEND_PROGRAM\n",
    );

    let analysis = core.analyze(&document);

    assert!(analysis.diagnostics().is_empty());
}

#[test]
fn compiler_core_detects_unclosed_block_comments() {
    let core = CompilerCore;
    let document = SourceDocument::new(
        "file:///main.st",
        1,
        "PROGRAM Main\n(* unfinished\nEND_PROGRAM",
    );

    let analysis = core.analyze(&document);

    assert_eq!(analysis.diagnostics().len(), 1);
    assert!(
        analysis.diagnostics()[0]
            .message
            .contains("Unclosed block comment")
    );
}

// Signature help (PLC-56): compiler-core exposes call signature data for the
// MVP standard functions and for user-declared functions / function blocks.

const PROGRAM_WITH_STANDARD_CALL: &str =
    "PROGRAM Main\nVAR\n    R : INT;\nEND_VAR\nR := MIN(1, 2);\nEND_PROGRAM\n";

#[test]
fn compiler_core_returns_signature_for_standard_function_call() {
    let core = CompilerCore;
    let document = SourceDocument::new("file:///main.st", 1, PROGRAM_WITH_STANDARD_CALL);

    // Cursor on the first argument of `MIN(1, 2)` (line 4, char 9).
    let signature = core
        .signature_help(
            &document,
            Position {
                line: 4,
                character: 9,
            },
        )
        .expect("signature for MIN");

    assert_eq!(signature.label, "MIN(IN1 : ANY_NUM; IN2 : ANY_NUM)");
    assert_eq!(signature.parameters.len(), 2);
    assert_eq!(signature.parameters[0].label, "IN1 : ANY_NUM");
    assert_eq!(signature.parameters[1].label, "IN2 : ANY_NUM");
    assert_eq!(signature.active_parameter, Some(0));
}

#[test]
fn compiler_core_tracks_active_parameter_after_comma() {
    let core = CompilerCore;
    let document = SourceDocument::new("file:///main.st", 1, PROGRAM_WITH_STANDARD_CALL);

    // Cursor on the second argument of `MIN(1, 2)` (line 4, char 12).
    let signature = core
        .signature_help(
            &document,
            Position {
                line: 4,
                character: 12,
            },
        )
        .expect("signature for MIN");

    assert_eq!(signature.active_parameter, Some(1));
}

const PROGRAM_WITH_FUNCTION_CALL: &str = concat!(
    "FUNCTION Add\n",
    "VAR_INPUT\n",
    "    A : INT;\n",
    "    B : INT;\n",
    "END_VAR\n",
    "END_FUNCTION\n",
    "PROGRAM Main\n",
    "VAR\n",
    "    R : INT;\n",
    "END_VAR\n",
    "R := Add(1, 2);\n",
    "END_PROGRAM\n",
);

#[test]
fn compiler_core_returns_signature_for_user_function_call() {
    let core = CompilerCore;
    let document = SourceDocument::new("file:///main.st", 1, PROGRAM_WITH_FUNCTION_CALL);

    // Cursor inside `Add(1, 2)` on line 10, char 9.
    let signature = core
        .signature_help(
            &document,
            Position {
                line: 10,
                character: 9,
            },
        )
        .expect("signature for Add");

    assert_eq!(signature.label, "Add(A : INT; B : INT)");
    assert_eq!(signature.parameters.len(), 2);
    assert_eq!(signature.parameters[0].label, "A : INT");
    assert_eq!(signature.active_parameter, Some(0));
}

const PROGRAM_WITH_FB_CALL: &str = concat!(
    "FUNCTION_BLOCK Counter\n",
    "VAR_INPUT\n",
    "    CU : BOOL;\n",
    "    PV : INT;\n",
    "END_VAR\n",
    "END_FUNCTION_BLOCK\n",
    "PROGRAM Main\n",
    "VAR\n",
    "    inst : Counter;\n",
    "END_VAR\n",
    "inst(CU := TRUE, PV := 10);\n",
    "END_PROGRAM\n",
);

#[test]
fn compiler_core_returns_signature_for_function_block_instance_call() {
    let core = CompilerCore;
    let document = SourceDocument::new("file:///main.st", 1, PROGRAM_WITH_FB_CALL);

    // Cursor inside `inst(...)` on line 10, char 5 (first input).
    let signature = core
        .signature_help(
            &document,
            Position {
                line: 10,
                character: 5,
            },
        )
        .expect("signature for Counter instance");

    assert_eq!(signature.label, "Counter(CU : BOOL; PV : INT)");
    assert_eq!(signature.parameters.len(), 2);
    assert_eq!(signature.parameters[0].label, "CU : BOOL");
    assert_eq!(signature.parameters[1].label, "PV : INT");
    assert_eq!(signature.active_parameter, Some(0));
}

#[test]
fn compiler_core_returns_no_signature_outside_call() {
    let core = CompilerCore;
    let document = SourceDocument::new("file:///main.st", 1, PROGRAM_WITH_STANDARD_CALL);

    // Cursor on `R` at the start of line 4 — not inside any call.
    assert!(
        core.signature_help(
            &document,
            Position {
                line: 4,
                character: 0,
            },
        )
        .is_none()
    );
}
