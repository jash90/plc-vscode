//! Semantic analysis for PLC VS Code.
//!
//! This crate builds the workspace symbol index from `plc_syntax` output and
//! resolves assignment targets against it. The API is intentionally
//! deterministic so it can later be backed by incremental queries without
//! changing CLI/LSP callers. A memoized query facade establishes the boundaries
//! a future salsa database can adopt directly.

mod query;
mod types;

use plc_syntax::{PouKind, StatementKind};

pub use query::{QueryDurability, QueryStats, SemanticQueryDatabase, SourceSnapshot};
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
    let mut diagnostics = Vec::new();
    let mut parsed_files = Vec::new();

    for file in files {
        let parsed = plc_syntax::parse_source(&file.text);
        index_file_symbols(file, &parsed, &mut symbol_index);
        parsed_files.push(parsed);
    }

    for parsed in &parsed_files {
        for unit in parsed.units() {
            let Some(container) = unit.name.as_deref() else {
                continue;
            };

            for statement in &unit.statements {
                if statement.kind != StatementKind::Assignment {
                    continue;
                }

                let Some(target) = statement.target.as_deref() else {
                    continue;
                };

                let Some(symbol) = symbol_index
                    .find_in_container(container, target)
                    .or_else(|| symbol_index.find_top_level(target))
                else {
                    diagnostics.push(SemanticDiagnostic {
                        code: "SEM0001",
                        range: statement.range,
                        message: format!("Unresolved symbol `{target}`"),
                    });
                    continue;
                };

                let Some(expected) = symbol.type_kind.as_ref() else {
                    continue;
                };
                let Some(expression) = statement.expression.as_deref() else {
                    continue;
                };
                let actual = infer_expression_type(expression, &symbol_index, container);
                if !expected.assignment_compatible(&actual) {
                    diagnostics.push(SemanticDiagnostic {
                        code: "SEM0002",
                        range: statement.range,
                        message: format!(
                            "Cannot assign {} expression to {} `{target}`",
                            actual.display_name(),
                            expected.display_name()
                        ),
                    });
                }
            }
        }
    }

    SemanticAnalysis {
        symbol_index,
        diagnostics,
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

fn infer_expression_type(expression: &str, index: &SymbolIndex, container: &str) -> TypeKind {
    let trimmed = expression.trim();
    let upper = trimmed.to_ascii_uppercase();

    if trimmed.starts_with('\'') || trimmed.starts_with('"') {
        TypeKind::String
    } else if matches!(upper.as_str(), "TRUE" | "FALSE") {
        TypeKind::Bool
    } else if upper.starts_with("T#") || upper.starts_with("TIME#") {
        TypeKind::Time
    } else if trimmed.parse::<i64>().is_ok() {
        TypeKind::Integer
    } else if trimmed.parse::<f64>().is_ok() {
        TypeKind::Real
    } else if let Some(symbol) = index
        .find_in_container(container, trimmed)
        .or_else(|| index.find_top_level(trimmed))
    {
        symbol
            .type_kind
            .clone()
            .unwrap_or_else(|| TypeKind::Unknown(trimmed.to_owned()))
    } else {
        TypeKind::Unknown(trimmed.to_owned())
    }
}
