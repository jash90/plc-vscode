//! Semantic analysis for PLC VS Code.
//!
//! This crate builds the first workspace symbol index from `plc_syntax` output.
//! The API is intentionally deterministic so it can later be backed by
//! incremental queries without changing CLI/LSP callers. Type checking and the
//! incremental query facade are added by later tasks.

mod types;

use plc_syntax::PouKind;

pub use types::{
    SemanticAnalysis, SemanticDiagnostic, SourceFile, Symbol, SymbolIndex, SymbolKind, TypeKind,
};

/// Analyze a single file.
pub fn analyze_file(uri: impl Into<String>, text: impl Into<String>) -> SemanticAnalysis {
    analyze_workspace(&[SourceFile::new(uri, text)])
}

/// Analyze a workspace snapshot and build a cross-file symbol index.
pub fn analyze_workspace(files: &[SourceFile]) -> SemanticAnalysis {
    let mut symbol_index = SymbolIndex::default();

    for file in files {
        let parsed = plc_syntax::parse_source(&file.text);
        index_file_symbols(file, &parsed, &mut symbol_index);
    }

    SemanticAnalysis {
        symbol_index,
        diagnostics: Vec::new(),
    }
}

fn index_file_symbols(
    file: &SourceFile,
    parsed: &plc_syntax::SyntaxParse,
    index: &mut SymbolIndex,
) {
    for unit in parsed.units() {
        if let Some(name) = unit.name.as_ref() {
            index.insert(Symbol {
                name: name.clone(),
                kind: symbol_kind_for_pou(unit.kind),
                type_kind: None,
                uri: file.uri.clone(),
                range: unit.range,
                container: None,
            });
        }

        let container = unit.name.clone();
        for block in &unit.declaration_blocks {
            for declaration in &block.declarations {
                index.insert(Symbol {
                    name: declaration.name.clone(),
                    kind: SymbolKind::Variable,
                    type_kind: Some(TypeKind::from_name(&declaration.type_name)),
                    uri: file.uri.clone(),
                    range: declaration.range,
                    container: container.clone(),
                });
            }
        }
    }
}

fn symbol_kind_for_pou(kind: PouKind) -> SymbolKind {
    match kind {
        PouKind::Program => SymbolKind::Program,
        PouKind::Function => SymbolKind::Function,
        PouKind::FunctionBlock => SymbolKind::FunctionBlock,
        PouKind::Action => SymbolKind::Action,
    }
}
