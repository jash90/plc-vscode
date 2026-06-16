//! Compiler-core public-API coverage exercised against the real `PRG_Test_ST.st`
//! (the existing `api_contract.rs` uses small synthetic programs).

use plc_compiler_core::{CompilerCore, Position, SemanticTokenKind, SourceDocument, SymbolKind};

const SOURCE: &str = include_str!("fixtures/prg_test_st.st");
const URI: &str = "file:///prg_test.st";

fn doc() -> SourceDocument {
    SourceDocument::new(URI, 1, SOURCE)
}

/// 0-based position of the start of `needle`'s first occurrence (executable
/// lines are ASCII, so char count == byte count on the target line).
fn find_pos(needle: &str) -> Position {
    let idx = SOURCE
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not found"));
    let line = SOURCE[..idx].matches('\n').count() as u32;
    let last_nl = SOURCE[..idx].rfind('\n').map(|p| p + 1).unwrap_or(0);
    let character = SOURCE[last_nl..idx].chars().count() as u32;
    Position { line, character }
}

/// Position just inside a call's parentheses (on its first argument).
fn pos_in_call(call: &str) -> Position {
    let p = find_pos(call);
    Position {
        line: p.line,
        character: p.character + call.chars().count() as u32,
    }
}

/// Position one char into `needle`'s first token (token-at-position resolution
/// picks the preceding whitespace token when the cursor sits at the exact start).
fn pos_on(needle: &str) -> Position {
    let p = find_pos(needle);
    Position {
        line: p.line,
        character: p.character + 1,
    }
}

// --- diagnostics ---

#[test]
fn analyze_has_zero_diagnostics() {
    let analysis = CompilerCore.analyze(&doc());
    assert!(
        analysis.diagnostics().is_empty(),
        "unexpected: {:?}",
        analysis.diagnostics()
    );
}

#[test]
fn analyze_propagates_version_metadata() {
    let analysis = CompilerCore.analyze(&SourceDocument::new(URI, 42, SOURCE));
    assert_eq!(analysis.uri(), URI);
    assert_eq!(analysis.version(), 42);
}

#[test]
fn analyze_emits_no_sem_codes() {
    let analysis = CompilerCore.analyze(&doc());
    assert!(analysis.diagnostics().iter().all(|d| d.code != "SEM0001"));
    assert!(analysis.diagnostics().iter().all(|d| d.code != "SEM0002"));
}

// --- document symbols ---

#[test]
fn document_symbols_single_program_with_58_members() {
    let symbols = CompilerCore.document_symbols(&doc());
    let symbols = symbols.symbols();
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].name, "PRG_Test");
    assert_eq!(symbols[0].kind, SymbolKind::Program);
    assert_eq!(symbols[0].children.len(), 58);
}

#[test]
fn document_symbols_member_details_reflect_types() {
    let analysis = CompilerCore.document_symbols(&doc());
    let children = &analysis.symbols()[0].children;
    let detail = |name: &str| {
        children
            .iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("member {name} missing"))
            .detail
            .clone()
    };
    assert_eq!(detail("sLog01").as_deref(), Some("STRING"));
    assert_eq!(detail("iA").as_deref(), Some("integer"));
    assert_eq!(detail("fbTON").as_deref(), Some("TON"));
    assert_eq!(detail("fbCTU").as_deref(), Some("CTU"));
}

#[test]
fn document_symbols_include_all_thirty_log_members() {
    let analysis = CompilerCore.document_symbols(&doc());
    let children = &analysis.symbols()[0].children;
    for n in 1..=30 {
        let name = format!("sLog{n:02}");
        assert!(children.iter().any(|c| c.name == name), "missing {name}");
    }
    assert!(children.iter().all(|c| c.kind == SymbolKind::Variable));
}

// --- completions ---

#[test]
fn completions_include_every_used_function() {
    let items = CompilerCore.completions(&doc(), Position::default());
    for name in [
        "EXPT",
        "SHL",
        "SHR",
        "SQRT",
        "ABS",
        "MAX",
        "MIN",
        "LIMIT",
        "SEL",
        "CONCAT",
        "LEN",
        "LEFT",
        "INT_TO_STRING",
        "REAL_TO_STRING",
        "BOOL_TO_STRING",
        "WORD_TO_STRING",
        "DINT_TO_STRING",
        "TIME_TO_STRING",
        "TO_STRING",
    ] {
        assert!(
            items
                .iter()
                .any(|c| c.label == name && c.kind == SymbolKind::Function),
            "completion missing function {name}"
        );
    }
}

#[test]
fn completions_include_standard_function_blocks() {
    let items = CompilerCore.completions(&doc(), Position::default());
    for name in ["TON", "CTU"] {
        assert!(
            items
                .iter()
                .any(|c| c.label == name && c.kind == SymbolKind::FunctionBlock),
            "completion missing FB {name}"
        );
    }
}

#[test]
fn completions_include_user_variables_and_keywords() {
    let items = CompilerCore.completions(&doc(), Position::default());
    for v in ["sLog01", "iA", "fbTON", "xInit"] {
        assert!(
            items
                .iter()
                .any(|c| c.label == v && c.kind == SymbolKind::Variable),
            "var {v}"
        );
    }
    // Structural keywords are offered as completions (operator-keywords like AND
    // are lexer keywords but are not part of the completion keyword catalog).
    for k in ["PROGRAM", "VAR", "IF", "FOR", "WHILE", "CASE", "RETURN"] {
        assert!(
            items
                .iter()
                .any(|c| c.label == k && c.kind == SymbolKind::Keyword),
            "kw {k}"
        );
    }
}

#[test]
fn completions_total_is_122() {
    assert_eq!(
        CompilerCore.completions(&doc(), Position::default()).len(),
        122
    );
}

// --- hover ---

#[test]
fn hover_on_program_keyword() {
    let pos = Position {
        line: find_pos("PROGRAM PRG_Test").line,
        character: 1,
    };
    let hover = CompilerCore.hover(&doc(), pos).expect("hover on PROGRAM");
    assert_eq!(hover.contents, "Structured Text keyword `PROGRAM`");
}

#[test]
fn hover_on_variable_shows_type() {
    let hover = CompilerCore
        .hover(&doc(), pos_on("iA + iB"))
        .expect("hover iA");
    assert_eq!(hover.contents, "iA: integer");
}

#[test]
fn hover_on_function_block_instance() {
    let hover = CompilerCore
        .hover(&doc(), pos_on("fbTON"))
        .expect("hover fbTON");
    assert_eq!(hover.contents, "fbTON: TON");
}

// --- signature help ---

#[test]
fn signature_help_for_expt() {
    let sig = CompilerCore
        .signature_help(&doc(), pos_in_call("EXPT("))
        .expect("EXPT sig");
    assert_eq!(sig.label, "EXPT(IN1 : ANY_NUM; IN2 : ANY_NUM)");
    assert_eq!(sig.parameters.len(), 2);
    assert_eq!(sig.active_parameter, Some(0));
}

#[test]
fn signature_help_for_limit_with_three_params() {
    let sig = CompilerCore
        .signature_help(&doc(), pos_in_call("LIMIT("))
        .expect("LIMIT sig");
    assert_eq!(sig.label, "LIMIT(MN : ANY_NUM; IN : ANY_NUM; MX : ANY_NUM)");
    assert_eq!(sig.parameters.len(), 3);
}

#[test]
fn signature_help_for_max_min_sel_concat() {
    let core = CompilerCore;
    assert_eq!(
        core.signature_help(&doc(), pos_in_call("MAX("))
            .unwrap()
            .label,
        "MAX(IN1 : ANY_NUM; IN2 : ANY_NUM)"
    );
    assert_eq!(
        core.signature_help(&doc(), pos_in_call("MIN("))
            .unwrap()
            .label,
        "MIN(IN1 : ANY_NUM; IN2 : ANY_NUM)"
    );
    assert_eq!(
        core.signature_help(&doc(), pos_in_call("SEL("))
            .unwrap()
            .parameters
            .len(),
        3
    );
    assert!(
        core.signature_help(&doc(), pos_in_call("CONCAT("))
            .unwrap()
            .label
            .starts_with("CONCAT(")
    );
}

#[test]
fn signature_help_none_at_statement_start() {
    assert!(
        CompilerCore
            .signature_help(&doc(), find_pos("iWynik := iA + iB"))
            .is_none()
    );
}

// --- semantic tokens ---

#[test]
fn semantic_tokens_total_is_1131() {
    assert_eq!(CompilerCore.semantic_tokens(&doc()).len(), 1131);
}

#[test]
fn semantic_tokens_first_token_is_program_keyword() {
    let tokens = CompilerCore.semantic_tokens(&doc());
    let program_line = find_pos("PROGRAM PRG_Test").line;
    let first = &tokens[0];
    assert_eq!(first.kind, SemanticTokenKind::Keyword);
    assert_eq!(first.range.start.line, program_line);
    assert_eq!(first.range.start.character, 0);
}

#[test]
fn semantic_tokens_cover_all_used_kinds() {
    let tokens = CompilerCore.semantic_tokens(&doc());
    let has = |k: SemanticTokenKind| tokens.iter().any(|t| t.kind == k);
    for kind in [
        SemanticTokenKind::Keyword,
        SemanticTokenKind::Type,
        SemanticTokenKind::Variable,
        SemanticTokenKind::Function,
        SemanticTokenKind::FunctionBlock,
        SemanticTokenKind::Number,
        SemanticTokenKind::String,
        SemanticTokenKind::Comment,
        SemanticTokenKind::Operator,
    ] {
        assert!(has(kind), "no token classified as {kind:?}");
    }
}

// --- formatting / code actions ---

#[test]
fn formatting_is_idempotent_on_the_real_file() {
    let core = CompilerCore;
    let edits = core.formatting(&doc());
    let formatted = match edits.first() {
        Some(edit) => edit.new_text.clone(),
        None => SOURCE.to_owned(),
    };
    let reformatted = core.formatting(&SourceDocument::new(URI, 1, formatted));
    assert!(reformatted.is_empty(), "formatting is not idempotent");
}

#[test]
fn code_actions_offers_no_quick_fix_on_clean_file() {
    let actions = CompilerCore.code_actions(&doc());
    assert!(actions.iter().all(|a| !a.title.contains("END_PROGRAM")));
}

// --- navigation ---

#[test]
fn definition_of_fb_usage_resolves_to_declaration() {
    let usage = pos_on("fbTON(IN");
    let decl_line = find_pos("fbTON").line; // first occurrence = declaration
    let location = CompilerCore.definition(&doc(), usage).expect("definition");
    assert_eq!(location.uri, URI);
    assert_eq!(location.range.start.line, decl_line);
}

#[test]
fn references_of_reused_variable_are_numerous() {
    let pos = pos_on("iWynik := iA + iB");
    let refs = CompilerCore.references(&doc(), pos, true);
    assert!(
        refs.len() >= 5,
        "expected many iWynik references, got {}",
        refs.len()
    );
}
