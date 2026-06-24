//! Structured Text frontend: the reference adapter over the existing crates.
//!
//! `lower` reuses `plc_hir::lower_source` for the IR and `CompilerCore::analyze`
//! for diagnostics (so ST conversion is gated by the same diagnostics the IDE
//! shows); `render` prints the IR back as ST. Analysis/IDE features delegate to
//! `CompilerCore`, so ST keeps every existing LSP feature unchanged and
//! `plc_compiler_core` stays runtime-free (rendering lives here, not there).

use std::sync::Arc;

use plc_api::{Analysis, LanguageService, SourceDocument};
use plc_compiler_core::CompilerCore;
use plc_hir::{BinaryOp, HirExpr, HirModule, HirPouKind, HirStmt, HirType, UnaryOp, lower_source};
use plc_syntax::{StatementKind, parse_source};

use crate::{LanguageFrontend, LoweringResult, RenderResult};

/// Structured Text (IEC 61131-3) language frontend.
pub struct StFrontend;

impl LanguageFrontend for StFrontend {
    fn id(&self) -> &'static str {
        "st"
    }

    fn display_name(&self) -> &'static str {
        "Structured Text"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["st", "iecst", "plcst"]
    }

    fn can_render(&self) -> bool {
        true
    }

    fn lower(&self, document: &SourceDocument) -> LoweringResult {
        let module = lower_source(document.text());
        let diagnostics = CompilerCore.analyze(document).diagnostics().to_vec();
        LoweringResult {
            module,
            diagnostics,
            fidelity: lowering_fidelity(document.text()),
        }
    }

    fn render(&self, module: &HirModule) -> RenderResult {
        render_structured_text(module)
    }

    fn analyze(&self, document: &SourceDocument) -> Analysis {
        CompilerCore.analyze(document)
    }

    fn language_service(&self) -> Option<Arc<dyn LanguageService + Send + Sync>> {
        Some(Arc::new(CompilerCore))
    }
}

/// Note source statements that the IR (assignment + `+`/`-` only) drops, so the
/// caller knows a conversion is partial rather than wrong.
fn lowering_fidelity(text: &str) -> Vec<String> {
    let parse = parse_source(text);
    let dropped = parse
        .units()
        .iter()
        .flat_map(|unit| unit.statements.iter())
        .filter(|statement| statement.kind != StatementKind::Assignment)
        .count();
    if dropped == 0 {
        Vec::new()
    } else {
        vec![format!(
            "{dropped} non-assignment statement(s) (control flow / calls) are not modeled by the IR and were dropped"
        )]
    }
}

fn render_structured_text(module: &HirModule) -> RenderResult {
    let mut out = String::new();
    let mut fidelity = Vec::new();

    for program in &module.programs {
        let (start_kw, end_kw) = pou_keywords(program.kind);
        out.push_str(start_kw);
        out.push(' ');
        out.push_str(&program.name);
        out.push('\n');

        if !program.vars.is_empty() {
            out.push_str("VAR\n");
            for var in &program.vars {
                let type_name = match hir_type_name(var.ty) {
                    Some(name) => name,
                    None => {
                        fidelity.push(format!(
                            "variable `{}` has a type not modeled by the IR; rendered as INT",
                            var.name
                        ));
                        "INT"
                    }
                };
                out.push_str(&format!("    {} : {};\n", var.name, type_name));
            }
            out.push_str("END_VAR\n");
        }

        for assign in &program.body {
            out.push_str(&format!(
                "    {} := {};\n",
                assign.target,
                render_expr(&assign.value)
            ));
        }

        for stmt in &program.statements {
            render_stmt(stmt, &mut out, &mut fidelity);
        }

        out.push_str(end_kw);
        out.push('\n');
    }

    RenderResult {
        text: out,
        fidelity,
    }
}

fn render_expr(expr: &HirExpr) -> String {
    match expr {
        HirExpr::Int(value) => value.to_string(),
        HirExpr::Real(value) => {
            // Keep a decimal point so it re-lexes as REAL.
            if value.fract() == 0.0 {
                format!("{value:.1}")
            } else {
                value.to_string()
            }
        }
        HirExpr::Bool(value) => if *value { "TRUE" } else { "FALSE" }.to_owned(),
        HirExpr::Str(value) => format!("'{value}'"),
        HirExpr::Var(name) => name.clone(),
        HirExpr::Binary { op, lhs, rhs } => {
            let operator = match op {
                BinaryOp::Add => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Mul => "*",
                BinaryOp::Div => "/",
                BinaryOp::Mod => "MOD",
                BinaryOp::And => "AND",
                BinaryOp::Or => "OR",
                BinaryOp::Xor => "XOR",
                BinaryOp::Eq => "=",
                BinaryOp::Ne => "<>",
                BinaryOp::Lt => "<",
                BinaryOp::Le => "<=",
                BinaryOp::Gt => ">",
                BinaryOp::Ge => ">=",
            };
            format!("{} {} {}", render_expr(lhs), operator, render_expr(rhs))
        }
        HirExpr::Unary { op, expr } => {
            let operator = match op {
                UnaryOp::Not => "NOT ",
                UnaryOp::Neg => "-",
            };
            format!("{operator}{}", render_expr(expr))
        }
        HirExpr::Call { name, args } => {
            let rendered_args: Vec<String> = args
                .iter()
                .map(|arg| match &arg.name {
                    Some(n) => format!("{n} := {}", render_expr(&arg.value)),
                    None => render_expr(&arg.value),
                })
                .collect();
            format!("{name}({})", rendered_args.join(", "))
        }
    }
}

fn render_stmt(stmt: &HirStmt, out: &mut String, fidelity: &mut Vec<String>) {
    match stmt {
        HirStmt::Assign(assign) => {
            out.push_str(&format!(
                "    {} := {};\n",
                assign.target,
                render_expr(&assign.value)
            ));
        }
        HirStmt::Set { target, value } => {
            out.push_str(&format!(
                "    IF {} THEN {target} := TRUE; END_IF;\n",
                render_expr(value)
            ));
        }
        HirStmt::Reset { target, value } => {
            out.push_str(&format!(
                "    IF {} THEN {target} := FALSE; END_IF;\n",
                render_expr(value)
            ));
        }
        HirStmt::FbCall {
            instance,
            fb_type,
            args,
        } => {
            let rendered_args: Vec<String> = args
                .iter()
                .map(|arg| match &arg.name {
                    Some(n) => format!("{n} := {}", render_expr(&arg.value)),
                    None => render_expr(&arg.value),
                })
                .collect();
            out.push_str(&format!(
                "    {instance}({}); (* {fb_type} *)\n",
                rendered_args.join(", ")
            ));
        }
    }
    let _ = fidelity; // reserved for future fidelity notes
}

fn pou_keywords(kind: HirPouKind) -> (&'static str, &'static str) {
    match kind {
        HirPouKind::Program => ("PROGRAM", "END_PROGRAM"),
        HirPouKind::Function => ("FUNCTION", "END_FUNCTION"),
        HirPouKind::FunctionBlock => ("FUNCTION_BLOCK", "END_FUNCTION_BLOCK"),
        HirPouKind::Action => ("ACTION", "END_ACTION"),
    }
}

fn hir_type_name(ty: HirType) -> Option<&'static str> {
    match ty {
        HirType::Bool => Some("BOOL"),
        HirType::Int => Some("INT"),
        HirType::Real => Some("REAL"),
        HirType::Str => Some("STRING"),
        HirType::Time => Some("TIME"),
        HirType::Unknown => None,
    }
}
