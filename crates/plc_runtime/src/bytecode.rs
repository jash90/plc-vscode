//! Serializable bytecode format and viewer contract.
//!
//! The MVP instruction set is a small stack machine: literals are pushed,
//! variables are loaded/stored by name, and a handful of arithmetic ops cover
//! the current expression surface. The format is serialized as JSON so it can
//! be persisted and rendered by an external VS Code bytecode viewer; the viewer
//! contract is the [`BytecodeModule::disassemble`] mnemonic listing.

use plc_hir::{BinaryOp, HirExpr, HirModule, HirProgram, UnaryOp};
use serde::{Deserialize, Serialize};

/// A single stack-machine instruction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Instruction {
    PushInt(i64),
    PushBool(bool),
    PushReal(f64),
    PushStr(String),
    LoadVar(String),
    StoreVar(String),
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    // Boolean logic
    And,
    Or,
    Xor,
    // Comparison
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    // Unary
    Not,
    Neg,
}

impl Instruction {
    /// Stable mnemonic + operand rendering used by the viewer contract.
    pub fn mnemonic(&self) -> String {
        match self {
            Instruction::PushInt(value) => format!("PUSH_INT {value}"),
            Instruction::PushBool(value) => format!("PUSH_BOOL {value}"),
            Instruction::PushReal(value) => format!("PUSH_REAL {value}"),
            Instruction::PushStr(value) => format!("PUSH_STR {value:?}"),
            Instruction::LoadVar(name) => format!("LOAD_VAR {name}"),
            Instruction::StoreVar(name) => format!("STORE_VAR {name}"),
            Instruction::Add => "ADD".to_owned(),
            Instruction::Sub => "SUB".to_owned(),
            Instruction::Mul => "MUL".to_owned(),
            Instruction::Div => "DIV".to_owned(),
            Instruction::Mod => "MOD".to_owned(),
            Instruction::And => "AND".to_owned(),
            Instruction::Or => "OR".to_owned(),
            Instruction::Xor => "XOR".to_owned(),
            Instruction::Eq => "EQ".to_owned(),
            Instruction::Ne => "NE".to_owned(),
            Instruction::Lt => "LT".to_owned(),
            Instruction::Le => "LE".to_owned(),
            Instruction::Gt => "GT".to_owned(),
            Instruction::Ge => "GE".to_owned(),
            Instruction::Not => "NOT".to_owned(),
            Instruction::Neg => "NEG".to_owned(),
        }
    }
}

/// A serializable bytecode module produced by lowering a program.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BytecodeModule {
    pub name: String,
    pub instructions: Vec<Instruction>,
}

impl BytecodeModule {
    pub fn new(name: impl Into<String>, instructions: Vec<Instruction>) -> Self {
        Self {
            name: name.into(),
            instructions,
        }
    }

    /// Serialize to the stable JSON wire format.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("bytecode module is serializable")
    }

    /// Deserialize from the JSON wire format.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Viewer contract: an indexed mnemonic listing for the bytecode viewer.
    pub fn disassemble(&self) -> Vec<String> {
        self.instructions
            .iter()
            .enumerate()
            .map(|(index, instruction)| format!("{index:04}  {}", instruction.mnemonic()))
            .collect()
    }
}

/// Lower a HIR program to a bytecode module (the VM-side consumer of HIR).
pub fn lower_program(program: &HirProgram) -> BytecodeModule {
    let mut instructions = Vec::new();
    for assign in &program.body {
        lower_expr(&assign.value, &mut instructions);
        instructions.push(Instruction::StoreVar(assign.target.clone()));
    }
    BytecodeModule::new(program.name.clone(), instructions)
}

/// Lower every program in a HIR module to its own bytecode module.
pub fn lower_module(module: &HirModule) -> Vec<BytecodeModule> {
    module.programs.iter().map(lower_program).collect()
}

fn lower_expr(expr: &HirExpr, out: &mut Vec<Instruction>) {
    match expr {
        HirExpr::Int(value) => out.push(Instruction::PushInt(*value)),
        HirExpr::Bool(value) => out.push(Instruction::PushBool(*value)),
        HirExpr::Real(value) => out.push(Instruction::PushReal(*value)),
        HirExpr::Str(value) => out.push(Instruction::PushStr(value.clone())),
        HirExpr::Var(name) => out.push(Instruction::LoadVar(name.clone())),
        HirExpr::Binary { op, lhs, rhs } => {
            lower_expr(lhs, out);
            lower_expr(rhs, out);
            out.push(match op {
                BinaryOp::Add => Instruction::Add,
                BinaryOp::Sub => Instruction::Sub,
                BinaryOp::Mul => Instruction::Mul,
                BinaryOp::Div => Instruction::Div,
                BinaryOp::Mod => Instruction::Mod,
                BinaryOp::And => Instruction::And,
                BinaryOp::Or => Instruction::Or,
                BinaryOp::Xor => Instruction::Xor,
                BinaryOp::Eq => Instruction::Eq,
                BinaryOp::Ne => Instruction::Ne,
                BinaryOp::Lt => Instruction::Lt,
                BinaryOp::Le => Instruction::Le,
                BinaryOp::Gt => Instruction::Gt,
                BinaryOp::Ge => Instruction::Ge,
            });
        }
        HirExpr::Unary { op, expr } => {
            lower_expr(expr, out);
            out.push(match op {
                UnaryOp::Not => Instruction::Not,
                UnaryOp::Neg => Instruction::Neg,
            });
        }
        // Calls are not lowered in the bytecode VM (they require the runtime
        // interpreter's FB state).  We emit a LoadVar of the instance name as a
        // best-effort so the module still serializes without panicking.
        HirExpr::Call { name, .. } => out.push(Instruction::LoadVar(name.clone())),
    }
}
