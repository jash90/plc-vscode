//! Backend-agnostic High-level Intermediate Representation (HIR) and lowering.
//!
//! The HIR sits between `plc_syntax` parse output and the execution backends.
//! It is intentionally backend-independent so the bytecode VM and the native
//! (LLVM) backend can both consume the same representation:
//!
//! - **Lowering** (`lower_source`) turns parsed POUs into [`HirModule`].
//! - **VM backend** walks the HIR to emit bytecode / interpret directly.
//! - **Native backend** walks the same HIR to emit LLVM IR.
//!
//! Keeping a single typed HIR avoids duplicating program structure in each
//! backend and gives both a common place to validate lowering.

use plc_syntax::{PouKind, StatementKind, parse_source};

/// HIR scalar type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirType {
    Bool,
    Int,
    Real,
    Str,
    Time,
    Unknown,
}

impl HirType {
    pub fn from_name(name: &str) -> Self {
        match name.trim().to_ascii_uppercase().as_str() {
            "BOOL" => HirType::Bool,
            "SINT" | "INT" | "DINT" | "LINT" | "USINT" | "UINT" | "UDINT" | "ULINT" => HirType::Int,
            "REAL" | "LREAL" => HirType::Real,
            "STRING" | "WSTRING" => HirType::Str,
            "TIME" | "DATE" | "TIME_OF_DAY" | "TOD" | "DATE_AND_TIME" | "DT" => HirType::Time,
            _ => HirType::Unknown,
        }
    }
}

/// Binary operators represented in the MVP HIR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
}

/// HIR expression.
#[derive(Debug, Clone, PartialEq)]
pub enum HirExpr {
    Int(i64),
    Real(f64),
    Bool(bool),
    Str(String),
    Var(String),
    Binary {
        op: BinaryOp,
        lhs: Box<HirExpr>,
        rhs: Box<HirExpr>,
    },
}

/// A declared variable with its lowered type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HirVar {
    pub name: String,
    pub ty: HirType,
}

/// An assignment statement in the HIR body.
#[derive(Debug, Clone, PartialEq)]
pub struct HirAssign {
    pub target: String,
    pub value: HirExpr,
}

/// The kind of program organization unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirPouKind {
    Program,
    Function,
    FunctionBlock,
    Action,
}

impl HirPouKind {
    fn from_syntax(kind: PouKind) -> Self {
        match kind {
            PouKind::Program => HirPouKind::Program,
            PouKind::Function => HirPouKind::Function,
            PouKind::FunctionBlock => HirPouKind::FunctionBlock,
            PouKind::Action => HirPouKind::Action,
        }
    }
}

/// A lowered program (POU).
#[derive(Debug, Clone, PartialEq)]
pub struct HirProgram {
    pub name: String,
    pub kind: HirPouKind,
    pub vars: Vec<HirVar>,
    pub body: Vec<HirAssign>,
}

/// A lowered module containing all programs in a source file.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HirModule {
    pub programs: Vec<HirProgram>,
}

/// Lower Structured Text source into backend-agnostic HIR.
pub fn lower_source(text: &str) -> HirModule {
    let parse = parse_source(text);
    let mut programs = Vec::new();

    for unit in parse.units() {
        let name = unit.name.clone().unwrap_or_default();
        let mut vars = Vec::new();
        for block in &unit.declaration_blocks {
            for declaration in &block.declarations {
                vars.push(HirVar {
                    name: declaration.name.clone(),
                    ty: HirType::from_name(&declaration.type_name),
                });
            }
        }

        let mut body = Vec::new();
        for statement in &unit.statements {
            if statement.kind != StatementKind::Assignment {
                continue;
            }
            if let (Some(target), Some(expression)) =
                (statement.target.as_deref(), statement.expression.as_deref())
            {
                body.push(HirAssign {
                    target: target.to_owned(),
                    value: lower_expression(expression),
                });
            }
        }

        programs.push(HirProgram {
            name,
            kind: HirPouKind::from_syntax(unit.kind),
            vars,
            body,
        });
    }

    HirModule { programs }
}

/// Lower an expression string into the MVP HIR expression grammar.
pub fn lower_expression(expression: &str) -> HirExpr {
    let trimmed = expression.trim();

    if let Some((left, right)) = split_binary(trimmed, '+') {
        return HirExpr::Binary {
            op: BinaryOp::Add,
            lhs: Box::new(lower_expression(left)),
            rhs: Box::new(lower_expression(right)),
        };
    }
    if let Some((left, right)) = split_binary(trimmed, '-') {
        return HirExpr::Binary {
            op: BinaryOp::Sub,
            lhs: Box::new(lower_expression(left)),
            rhs: Box::new(lower_expression(right)),
        };
    }
    lower_operand(trimmed)
}

fn lower_operand(token: &str) -> HirExpr {
    let token = token.trim();
    let upper = token.to_ascii_uppercase();
    if upper == "TRUE" {
        return HirExpr::Bool(true);
    }
    if upper == "FALSE" {
        return HirExpr::Bool(false);
    }
    if token.starts_with('\'') && token.ends_with('\'') && token.len() >= 2 {
        return HirExpr::Str(token[1..token.len() - 1].to_owned());
    }
    if let Ok(int) = token.parse::<i64>() {
        return HirExpr::Int(int);
    }
    if let Ok(real) = token.parse::<f64>() {
        return HirExpr::Real(real);
    }
    HirExpr::Var(token.to_owned())
}

fn split_binary(expression: &str, op: char) -> Option<(&str, &str)> {
    let index = expression.find(op)?;
    if index == 0 {
        return None;
    }
    let (left, right) = expression.split_at(index);
    Some((left, &right[op.len_utf8()..]))
}
