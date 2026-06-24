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

/// Binary operators represented in the HIR.
///
/// Extended from the original `Add`/`Sub` MVP to support Ladder Diagram
/// lowering (boolean logic) and round-trip ST conversion covering the full
/// IEC 61131-3 expression surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    // Arithmetic (original)
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    // Boolean logic (LD lowering)
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
}

/// Unary operators represented in the HIR (negation and logical NOT).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    Neg,
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
    /// Unary operator application (`NOT A`, `-A`).
    Unary {
        op: UnaryOp,
        expr: Box<HirExpr>,
    },
    /// Function or function-block call (`TON(IN := x, PT := T#2s)`).
    Call {
        name: String,
        args: Vec<HirCallArg>,
    },
}

/// A named or positional call argument in the HIR.
#[derive(Debug, Clone, PartialEq)]
pub struct HirCallArg {
    pub name: Option<String>,
    pub value: HirExpr,
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

/// A statement in the HIR body.
///
/// The original IR only modeled plain assignments (`HirAssign`).  Ladder
/// Diagram needs SET/RESET coils and function-block invocations, which are
/// surfaced here as additional statement kinds so LD can lower to the same
/// canonical IR without losing semantics.
#[derive(Debug, Clone, PartialEq)]
pub enum HirStmt {
    Assign(HirAssign),
    /// SET coil (`S` variant) — force a variable to TRUE.
    Set { target: String, value: HirExpr },
    /// RESET coil (`R` variant) — force a variable to FALSE when condition is met.
    Reset { target: String, value: HirExpr },
    /// Function-block call (`TON_inst(IN := x, PT := T#2s);`).
    FbCall {
        instance: String,
        fb_type: String,
        args: Vec<HirCallArg>,
    },
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
    /// Plain assignments (the original IR body — assignments from ST source).
    pub body: Vec<HirAssign>,
    /// Extended statements (SET/RESET coils, FB calls — from LD lowering).
    pub statements: Vec<HirStmt>,
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
            statements: Vec::new(),
        });
    }

    HirModule { programs }
}

/// Lower an expression string into the HIR expression grammar.
///
/// Uses operator precedence climbing so multi-operator expressions like
/// `A AND B OR NOT C` lower correctly.  Unsupported tokens become `HirExpr::Var`
/// so partial coverage degrades gracefully (matching the original contract).
pub fn lower_expression(expression: &str) -> HirExpr {
    let tokens = tokenize_expr(expression.trim());
    let mut parser = ExprParser { tokens, pos: 0 };
    let result = parser.parse_binary(0);
    result.unwrap_or_else(|| lower_operand(expression.trim()))
}

/// Minimal token for expression lowering.
#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Word(String),   // identifier, keyword, or literal
    Op(String),     // operator symbol
}

/// Tokenize an expression string into words and operators.
fn tokenize_expr(s: &str) -> Vec<Tok> {
    let mut tokens = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c.is_alphabetic() || c == '_' {
            let mut word = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_alphanumeric() || c == '_' || c == '.' {
                    word.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            tokens.push(Tok::Word(word));
        } else if c.is_ascii_digit() {
            let mut word = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_digit() || c == '.' {
                    word.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            tokens.push(Tok::Word(word));
        } else if c == '\'' {
            // String literal
            let mut word = String::new();
            word.push(c);
            chars.next();
            while let Some(&sc) = chars.peek() {
                word.push(sc);
                chars.next();
                if sc == '\'' {
                    break;
                }
            }
            tokens.push(Tok::Word(word));
        } else if c == '#' {
            // Typed literal like T#2s — consume the whole thing as a word
            let mut word = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_alphanumeric() || c == '#' || c == '.' {
                    word.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            tokens.push(Tok::Word(word));
        } else {
            // Single-char or two-char operator
            let op = c.to_string();
            chars.next();
            tokens.push(Tok::Op(op));
        }
    }
    tokens
}

/// Precedence-based recursive-descent expression parser over tokenized text.
struct ExprParser {
    tokens: Vec<Tok>,
    pos: usize,
}

impl ExprParser {
    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.pos)
    }

    /// Parse a binary expression with precedence >= `min_prec`.
    fn parse_binary(&mut self, min_prec: u8) -> Option<HirExpr> {
        let mut left = self.parse_unary()?;
        while let Some(tok) = self.peek() {
            let (op, prec) = match self.token_binary_op(tok) {
                Some((op, prec)) if prec >= min_prec => (op, prec),
                _ => break,
            };
            self.pos += 1;
            let right = self.parse_binary(prec + 1)?;
            left = HirExpr::Binary {
                op,
                lhs: Box::new(left),
                rhs: Box::new(right),
            };
        }
        Some(left)
    }

    /// Parse a unary prefix (`NOT`, `-`).
    fn parse_unary(&mut self) -> Option<HirExpr> {
        match self.peek() {
            Some(Tok::Word(w)) if w.eq_ignore_ascii_case("NOT") => {
                self.pos += 1;
                let expr = self.parse_unary()?;
                Some(HirExpr::Unary {
                    op: UnaryOp::Not,
                    expr: Box::new(expr),
                })
            }
            Some(Tok::Op(o)) if o == "-" => {
                self.pos += 1;
                let expr = self.parse_unary()?;
                Some(HirExpr::Unary {
                    op: UnaryOp::Neg,
                    expr: Box::new(expr),
                })
            }
            _ => self.parse_primary(),
        }
    }

    /// Parse a primary (literal, variable, parenthesized expression, or call).
    fn parse_primary(&mut self) -> Option<HirExpr> {
        match self.peek()?.clone() {
            Tok::Op(o) if o == "(" => {
                self.pos += 1;
                let inner = self.parse_binary(0)?;
                if let Some(Tok::Op(o)) = self.peek() {
                    if o == ")" {
                        self.pos += 1;
                    }
                }
                Some(inner)
            }
            Tok::Word(name) => {
                self.pos += 1;
                // Check for call: Word followed by '('
                if let Some(Tok::Op(o)) = self.peek() {
                    if o == "(" {
                        self.pos += 1;
                        let args = self.parse_call_args();
                        return Some(HirExpr::Call { name, args });
                    }
                }
                Some(lower_operand(&name))
            }
            _ => None,
        }
    }

    /// Parse call arguments until closing paren.
    fn parse_call_args(&mut self) -> Vec<HirCallArg> {
        let mut args = Vec::new();
        loop {
            // Check for named arg: Word ':='
            let mut named = false;
            let name;
            let saved_pos = self.pos;
            if let Some(Tok::Word(w)) = self.peek().cloned() {
                if let Some(Tok::Op(o)) = self.tokens.get(self.pos + 1) {
                    if o == ":=" {
                        name = Some(w);
                        self.pos += 2;
                        named = true;
                    } else {
                        name = None;
                    }
                } else {
                    name = None;
                }
            } else {
                name = None;
            }
            if !named {
                self.pos = saved_pos;
            }
            if let Some(value) = self.parse_binary(0) {
                args.push(HirCallArg { name, value });
            }
            match self.peek() {
                Some(Tok::Op(o)) if o == "," => {
                    self.pos += 1;
                    continue;
                }
                Some(Tok::Op(o)) if o == ")" => {
                    self.pos += 1;
                    break;
                }
                _ => break,
            }
        }
        args
    }

    /// Map a token to a binary operator with its precedence (0 = loosest).
    fn token_binary_op(&self, tok: &Tok) -> Option<(BinaryOp, u8)> {
        match tok {
            Tok::Word(w) => {
                let upper = w.to_ascii_uppercase();
                match upper.as_str() {
                    "OR" => Some((BinaryOp::Or, 1)),
                    "XOR" => Some((BinaryOp::Xor, 2)),
                    "AND" => Some((BinaryOp::And, 3)),
                    "MOD" => Some((BinaryOp::Mod, 7)),
                    _ => None,
                }
            }
            Tok::Op(o) => match o.as_str() {
                "=" => Some((BinaryOp::Eq, 4)),
                "<>" => Some((BinaryOp::Ne, 4)),
                "<" => Some((BinaryOp::Lt, 5)),
                "<=" => Some((BinaryOp::Le, 5)),
                ">" => Some((BinaryOp::Gt, 5)),
                ">=" => Some((BinaryOp::Ge, 5)),
                "+" => Some((BinaryOp::Add, 6)),
                "-" => Some((BinaryOp::Sub, 6)),
                "*" => Some((BinaryOp::Mul, 7)),
                "/" => Some((BinaryOp::Div, 7)),
                _ => None,
            },
        }
    }
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

