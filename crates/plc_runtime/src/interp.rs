//! Tree-walking interpreter for the Structured Text execution subset.
//!
//! The error-tolerant parser facade in `plc_syntax` only exposes a *flat* list
//! of statement summaries (control-flow keywords become empty markers and
//! expressions stay as joined token text), which is enough for IDE features but
//! cannot be executed. This module builds a real nested AST directly from the
//! lexer token stream and evaluates it deterministically over [`Value`] state,
//! so `IF`/`CASE`/`FOR`/`WHILE`/`REPEAT`, full expressions (operator precedence,
//! function calls, member access), and standard function-block calls all run.
//!
//! It deliberately reuses the existing lexer (`plc_syntax::lex_source`) and the
//! standard library (`crate::stdlib`) and function-block primitives
//! (`crate::timers` / `counters` / `edge`) instead of duplicating them.

use std::collections::HashMap;

use plc_syntax::{PouKind, Token, TokenKind, parse_source};

use crate::counters::{Ctd, Ctu, Ctud};
use crate::edge::{FTrig, RTrig};
use crate::timers::{Tof, Ton, Tp};
use crate::{Value, VariableTable, stdlib};

// ---------------------------------------------------------------------------
// AST
// ---------------------------------------------------------------------------

/// Binary operator, ordered loosely by family rather than precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Or,
    Xor,
    And,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Not,
    Neg,
}

/// An executable Structured Text expression.
#[derive(Debug, Clone)]
pub enum Expr {
    Lit(Value),
    Var(String),
    /// Function-block / struct member read (`inst.Q`).
    Member(Box<Expr>, String),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// Standard-function call (`SQRT(x)`, `CONCAT(a, b)`).
    Call(String, Vec<Expr>),
}

/// A single CASE label (`2`) or inclusive range (`1..5`).
#[derive(Debug, Clone, Copy)]
pub enum CaseLabel {
    Single(i64),
    Range(i64, i64),
}

/// One actual argument of a function-block call: named (`IN := x`) or positional.
#[derive(Debug, Clone)]
pub struct CallArg {
    pub name: Option<String>,
    pub value: Expr,
}

/// An executable statement, tagged with the 1-based source line of its first
/// token so the stepping debugger can map execution to editor positions.
#[derive(Debug, Clone)]
pub struct Stmt {
    pub line: u32,
    pub kind: StmtKind,
}

/// The executable shape of a statement (without its source position).
#[derive(Debug, Clone)]
pub enum StmtKind {
    Assign {
        target: String,
        value: Expr,
    },
    /// Function-block invocation statement (`fbTON(IN := x, PT := T#2s);`).
    FbCall {
        instance: String,
        args: Vec<CallArg>,
    },
    If {
        branches: Vec<(Expr, Vec<Stmt>)>,
        else_body: Vec<Stmt>,
    },
    Case {
        selector: Expr,
        branches: Vec<(Vec<CaseLabel>, Vec<Stmt>)>,
        else_body: Vec<Stmt>,
    },
    For {
        var: String,
        from: Expr,
        to: Expr,
        by: Option<Expr>,
        body: Vec<Stmt>,
    },
    While {
        cond: Expr,
        body: Vec<Stmt>,
    },
    Repeat {
        body: Vec<Stmt>,
        until: Expr,
    },
    Return,
    Exit,
    Continue,
}

/// A declared variable lowered for runtime initialization.
#[derive(Debug, Clone)]
pub struct VarDecl {
    pub name: String,
    pub type_name: String,
    /// Sizing clause spelling for sized types (`Some("[80]")` for `STRING[80]`),
    /// or `None` for an unsized type. Backends parse the capacity from this.
    pub type_size: Option<String>,
    pub init: Option<Value>,
    /// `true` for a `VAR_INPUT` declaration.
    pub is_input: bool,
    /// `true` for a `VAR_OUTPUT` declaration.
    pub is_output: bool,
    /// `true` when the declared type is a standard function block (TON, CTU, …).
    pub is_fb: bool,
}

/// A lowered, executable program: its declarations and its statement body.
#[derive(Debug, Clone)]
pub struct Program {
    pub vars: Vec<VarDecl>,
    pub body: Vec<Stmt>,
}

/// A single lowered program organization unit (PROGRAM / FUNCTION_BLOCK / …):
/// its kind, declarations, and statement body. Produced by [`build_units`] for
/// code-generation backends that need per-POU bodies (the interpreter merges
/// PROGRAM bodies via [`build_program`]).
#[derive(Debug, Clone)]
pub struct Unit {
    pub name: String,
    pub kind: plc_syntax::PouKind,
    pub vars: Vec<VarDecl>,
    pub body: Vec<Stmt>,
}

/// Maps a byte offset in the source to its 1-based line number, so statements
/// can be stamped with editor-facing line positions (DAP / VS Code are 1-based).
pub(crate) struct LineIndex {
    /// Byte offset of the start of each line, in ascending order (line 1 = 0).
    line_starts: Vec<usize>,
}

impl LineIndex {
    fn new(text: &str) -> Self {
        let mut line_starts = vec![0usize];
        for (offset, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(offset + 1);
            }
        }
        Self { line_starts }
    }

    /// 1-based line containing `offset`.
    fn line_of(&self, offset: usize) -> u32 {
        match self.line_starts.binary_search(&offset) {
            Ok(index) => (index + 1) as u32,
            Err(index) => index as u32,
        }
    }
}

// ---------------------------------------------------------------------------
// Program building (declarations via parse_source, body via the lexer)
// ---------------------------------------------------------------------------

/// Build an executable [`Program`] from Structured Text source. Declarations are
/// taken from the parser facade (so they match the rest of the toolchain);
/// `PROGRAM` bodies are re-parsed into a real AST from the lexer token stream.
pub fn build_program(text: &str) -> Program {
    let parse = parse_source(text);
    let mut vars = Vec::new();

    for unit in parse.units() {
        vars.extend(unit_vars(unit));
    }

    let body_tokens = program_body_tokens(&parse);
    let line_index = LineIndex::new(text);
    let mut parser = StmtParser::new(&body_tokens, &line_index);
    let body = parser.parse_block(&[]);

    Program { vars, body }
}

/// Lower one parsed unit's declaration blocks into [`VarDecl`]s.
fn unit_vars(unit: &plc_syntax::Pou) -> Vec<VarDecl> {
    let mut vars = Vec::new();
    for block in &unit.declaration_blocks {
        let is_input = block.kind == plc_syntax::VarBlockKind::Input;
        let is_output = block.kind == plc_syntax::VarBlockKind::Output;
        for declaration in &block.declarations {
            let is_fb = is_standard_fb(&declaration.type_name);
            let init = declaration
                .initializer
                .as_deref()
                .and_then(Value::parse_literal);
            vars.push(VarDecl {
                name: declaration.name.clone(),
                type_name: declaration.type_name.clone(),
                type_size: declaration.type_size.clone(),
                init,
                is_input,
                is_output,
                is_fb,
            });
        }
    }
    vars
}

/// Lower every program organization unit (PROGRAM, FUNCTION_BLOCK, …) into a
/// [`Unit`] with its own declarations and parsed statement body. Backends that
/// generate per-POU code (e.g. function blocks) consume this; the interpreter
/// uses [`build_program`], which only runs PROGRAM bodies.
pub fn build_units(text: &str) -> Vec<Unit> {
    let parse = parse_source(text);
    let line_index = LineIndex::new(text);
    let mut units = Vec::new();
    for unit in parse.units() {
        let name = unit.name.clone().unwrap_or_default();
        let body_tokens = unit_body_tokens(&parse, unit.kind, &name);
        let mut parser = StmtParser::new(&body_tokens, &line_index);
        units.push(Unit {
            name,
            kind: unit.kind,
            vars: unit_vars(unit),
            body: parser.parse_block(&[]),
        });
    }
    units
}

/// Collect the significant body tokens of the POU with the given kind and name,
/// excluding its header, its `VAR…END_VAR` blocks, and its terminator.
fn unit_body_tokens(parse: &plc_syntax::SyntaxParse, kind: PouKind, name: &str) -> Vec<Token> {
    let significant: Vec<&Token> = parse
        .tokens()
        .iter()
        .filter(|token| !token.is_trivia())
        .collect();
    let start_kw = kind.start_keyword();
    let end_kw = kind.end_keyword();

    let mut out = Vec::new();
    let mut cursor = 0usize;
    while cursor < significant.len() {
        // Find `<start_kw> <name>`.
        if !significant[cursor].keyword_eq(start_kw) {
            cursor += 1;
            continue;
        }
        cursor += 1;
        let matches_name = significant
            .get(cursor)
            .is_some_and(|token| token.kind == TokenKind::Identifier && token.text == name);
        if significant
            .get(cursor)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
        {
            cursor += 1;
        }
        while cursor < significant.len() && !significant[cursor].keyword_eq(end_kw) {
            if is_var_block_keyword(significant[cursor]) {
                cursor += 1;
                while cursor < significant.len() && !significant[cursor].keyword_eq("END_VAR") {
                    cursor += 1;
                }
                cursor += 1; // consume END_VAR
            } else {
                if matches_name {
                    out.push(significant[cursor].clone());
                }
                cursor += 1;
            }
        }
        cursor += 1; // consume the terminator
        if matches_name {
            break;
        }
    }
    out
}

/// Collect the significant body tokens of every `PROGRAM` unit, excluding the
/// header, the `VAR…END_VAR` declaration blocks, and the terminator.
fn program_body_tokens(parse: &plc_syntax::SyntaxParse) -> Vec<Token> {
    let significant: Vec<&Token> = parse
        .tokens()
        .iter()
        .filter(|token| !token.is_trivia())
        .collect();

    let mut out = Vec::new();
    let mut cursor = 0usize;
    while cursor < significant.len() {
        if !significant[cursor].keyword_eq("PROGRAM") {
            cursor += 1;
            continue;
        }
        cursor += 1;
        if significant
            .get(cursor)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
        {
            cursor += 1;
        }
        while cursor < significant.len() && !significant[cursor].keyword_eq("END_PROGRAM") {
            if is_var_block_keyword(significant[cursor]) {
                cursor += 1;
                while cursor < significant.len() && !significant[cursor].keyword_eq("END_VAR") {
                    cursor += 1;
                }
                cursor += 1; // consume END_VAR (or fall off the end)
            } else {
                out.push(significant[cursor].clone());
                cursor += 1;
            }
        }
        cursor += 1; // consume END_PROGRAM
    }
    out
}

fn is_var_block_keyword(token: &Token) -> bool {
    [
        "VAR",
        "VAR_INPUT",
        "VAR_OUTPUT",
        "VAR_IN_OUT",
        "VAR_GLOBAL",
        "VAR_TEMP",
        "VAR_EXTERNAL",
    ]
    .iter()
    .any(|keyword| token.keyword_eq(keyword))
}

fn is_standard_fb(type_name: &str) -> bool {
    matches!(
        type_name.to_ascii_uppercase().as_str(),
        "TON" | "TOF" | "TP" | "CTU" | "CTD" | "CTUD" | "R_TRIG" | "F_TRIG"
    )
}

// ---------------------------------------------------------------------------
// Recursive-descent statement / expression parser
// ---------------------------------------------------------------------------

struct StmtParser<'a> {
    tokens: &'a [Token],
    line_index: &'a LineIndex,
    pos: usize,
}

impl<'a> StmtParser<'a> {
    fn new(tokens: &'a [Token], line_index: &'a LineIndex) -> Self {
        Self {
            tokens,
            line_index,
            pos: 0,
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.pos);
        if token.is_some() {
            self.pos += 1;
        }
        token
    }

    fn at_keyword(&self, keyword: &str) -> bool {
        self.peek().is_some_and(|token| token.keyword_eq(keyword))
    }

    fn at_text(&self, text: &str) -> bool {
        self.peek().is_some_and(|token| token.text == text)
    }

    fn eat_text(&mut self, text: &str) -> bool {
        if self.at_text(text) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn eat_keyword(&mut self, keyword: &str) -> bool {
        if self.at_keyword(keyword) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// Consume an optional trailing `;` after a statement or block.
    fn eat_semicolon(&mut self) {
        self.eat_text(";");
    }

    /// Parse statements until a terminator keyword (or end of input).
    fn parse_block(&mut self, terminators: &[&str]) -> Vec<Stmt> {
        let mut statements = Vec::new();
        while let Some(token) = self.peek() {
            if terminators.iter().any(|term| token.keyword_eq(term)) {
                break;
            }
            let before = self.pos;
            if let Some(stmt) = self.parse_statement() {
                statements.push(stmt);
            }
            // Guarantee forward progress on unrecognized input.
            if self.pos == before {
                self.pos += 1;
            }
        }
        statements
    }

    /// Parse one statement, stamping it with the 1-based source line of its
    /// first token. The line is captured *before* parsing so control-flow
    /// statements take the line of their head keyword.
    fn parse_statement(&mut self) -> Option<Stmt> {
        let line = self
            .peek()
            .map(|token| self.line_index.line_of(token.range.start))
            .unwrap_or(0);
        let kind = self.parse_statement_kind()?;
        Some(Stmt { line, kind })
    }

    fn parse_statement_kind(&mut self) -> Option<StmtKind> {
        let token = self.peek()?;
        if token.kind == TokenKind::Keyword {
            return match token.text.to_ascii_uppercase().as_str() {
                "IF" => self.parse_if(),
                "CASE" => self.parse_case(),
                "FOR" => self.parse_for(),
                "WHILE" => self.parse_while(),
                "REPEAT" => self.parse_repeat(),
                "RETURN" => {
                    self.pos += 1;
                    self.eat_semicolon();
                    Some(StmtKind::Return)
                }
                "EXIT" => {
                    self.pos += 1;
                    self.eat_semicolon();
                    Some(StmtKind::Exit)
                }
                "CONTINUE" => {
                    self.pos += 1;
                    self.eat_semicolon();
                    Some(StmtKind::Continue)
                }
                _ => {
                    self.pos += 1;
                    None
                }
            };
        }

        if token.kind == TokenKind::Identifier {
            let name = token.text.clone();
            let next = self.tokens.get(self.pos + 1);
            if next.is_some_and(|token| token.text == ":=") {
                self.pos += 2; // identifier + :=
                let value = self.parse_expr();
                self.eat_semicolon();
                return Some(StmtKind::Assign {
                    target: name,
                    value,
                });
            }
            if next.is_some_and(|token| token.text == "(") {
                self.pos += 1; // identifier (leave '(' for arg parser)
                let args = self.parse_call_args();
                self.eat_semicolon();
                return Some(StmtKind::FbCall {
                    instance: name,
                    args,
                });
            }
            // Member/index assignment target or otherwise unsupported — skip the
            // whole statement so it cannot derail the following ones.
            self.skip_to_semicolon();
            return None;
        }

        // Stray operator/literal at statement position: recover by skipping it.
        self.pos += 1;
        None
    }

    fn skip_to_semicolon(&mut self) {
        while self.pos < self.tokens.len() {
            let is_semicolon = self.tokens[self.pos].text == ";";
            self.pos += 1;
            if is_semicolon {
                break;
            }
        }
    }

    fn parse_if(&mut self) -> Option<StmtKind> {
        self.eat_keyword("IF");
        let cond = self.parse_expr();
        self.eat_keyword("THEN");
        let body = self.parse_block(&["ELSIF", "ELSE", "END_IF"]);
        let mut branches = vec![(cond, body)];

        while self.eat_keyword("ELSIF") {
            let cond = self.parse_expr();
            self.eat_keyword("THEN");
            let body = self.parse_block(&["ELSIF", "ELSE", "END_IF"]);
            branches.push((cond, body));
        }

        let else_body = if self.eat_keyword("ELSE") {
            self.parse_block(&["END_IF"])
        } else {
            Vec::new()
        };

        self.eat_keyword("END_IF");
        self.eat_semicolon();
        Some(StmtKind::If {
            branches,
            else_body,
        })
    }

    fn parse_case(&mut self) -> Option<StmtKind> {
        self.eat_keyword("CASE");
        let selector = self.parse_expr();
        self.eat_keyword("OF");

        let mut branches = Vec::new();
        let mut else_body = Vec::new();
        while let Some(token) = self.peek() {
            if token.keyword_eq("END_CASE") {
                break;
            }
            if token.keyword_eq("ELSE") {
                self.pos += 1;
                else_body = self.parse_block(&["END_CASE"]);
                break;
            }
            let labels = self.parse_case_labels();
            let body = self.parse_case_branch_body();
            branches.push((labels, body));
        }

        self.eat_keyword("END_CASE");
        self.eat_semicolon();
        Some(StmtKind::Case {
            selector,
            branches,
            else_body,
        })
    }

    fn parse_case_labels(&mut self) -> Vec<CaseLabel> {
        let mut labels = Vec::new();
        while let Some(first) = self.parse_int_constant() {
            if self.eat_text("..") {
                let second = self.parse_int_constant().unwrap_or(first);
                labels.push(CaseLabel::Range(first, second));
            } else {
                labels.push(CaseLabel::Single(first));
            }
            if !self.eat_text(",") {
                break;
            }
        }
        self.eat_text(":");
        labels
    }

    /// Parse a CASE branch body: statements until the next label, `ELSE`, or
    /// `END_CASE`.
    fn parse_case_branch_body(&mut self) -> Vec<Stmt> {
        let mut statements = Vec::new();
        while let Some(token) = self.peek() {
            if token.keyword_eq("END_CASE") || token.keyword_eq("ELSE") {
                break;
            }
            if self.looks_like_case_label() {
                break;
            }
            let before = self.pos;
            if let Some(stmt) = self.parse_statement() {
                statements.push(stmt);
            }
            if self.pos == before {
                self.pos += 1;
            }
        }
        statements
    }

    /// Heuristic: the upcoming tokens form a CASE label (`<const>[,<const>][..]:`)
    /// rather than a statement, i.e. a `:` is reached before any `;` or `:=`.
    fn looks_like_case_label(&self) -> bool {
        let mut cursor = self.pos;
        while let Some(token) = self.tokens.get(cursor) {
            match token.text.as_str() {
                ":" => return true,
                ":=" | ";" => return false,
                "," | ".." | "-" => {}
                _ => {
                    if !matches!(token.kind, TokenKind::NumberLiteral | TokenKind::Identifier) {
                        return false;
                    }
                }
            }
            cursor += 1;
        }
        false
    }

    fn parse_int_constant(&mut self) -> Option<i64> {
        let negative = self.eat_text("-");
        let token = self.peek()?;
        let magnitude = match token.kind {
            TokenKind::NumberLiteral => match Value::parse_literal(&token.text) {
                Some(Value::Int(value)) => value,
                _ => return None,
            },
            _ => return None,
        };
        self.pos += 1;
        Some(if negative { -magnitude } else { magnitude })
    }

    fn parse_for(&mut self) -> Option<StmtKind> {
        self.eat_keyword("FOR");
        let var = self.advance()?.text.clone();
        self.eat_text(":=");
        let from = self.parse_expr();
        self.eat_keyword("TO");
        let to = self.parse_expr();
        let by = if self.eat_keyword("BY") {
            Some(self.parse_expr())
        } else {
            None
        };
        self.eat_keyword("DO");
        let body = self.parse_block(&["END_FOR"]);
        self.eat_keyword("END_FOR");
        self.eat_semicolon();
        Some(StmtKind::For {
            var,
            from,
            to,
            by,
            body,
        })
    }

    fn parse_while(&mut self) -> Option<StmtKind> {
        self.eat_keyword("WHILE");
        let cond = self.parse_expr();
        self.eat_keyword("DO");
        let body = self.parse_block(&["END_WHILE"]);
        self.eat_keyword("END_WHILE");
        self.eat_semicolon();
        Some(StmtKind::While { cond, body })
    }

    fn parse_repeat(&mut self) -> Option<StmtKind> {
        self.eat_keyword("REPEAT");
        let body = self.parse_block(&["UNTIL"]);
        self.eat_keyword("UNTIL");
        let until = self.parse_expr();
        self.eat_keyword("END_REPEAT");
        self.eat_semicolon();
        Some(StmtKind::Repeat { body, until })
    }

    // -- expressions (precedence climbing) ---------------------------------

    fn parse_expr(&mut self) -> Expr {
        self.parse_binary(0)
    }

    fn parse_binary(&mut self, min_prec: u8) -> Expr {
        let mut lhs = self.parse_unary();
        while let Some((op, prec, right_assoc)) = self.peek_binop() {
            if prec < min_prec {
                break;
            }
            self.pos += 1;
            let next_min = if right_assoc { prec } else { prec + 1 };
            let rhs = self.parse_binary(next_min);
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs));
        }
        lhs
    }

    fn peek_binop(&self) -> Option<(BinOp, u8, bool)> {
        let token = self.peek()?;
        if token.kind == TokenKind::Keyword {
            return match token.text.to_ascii_uppercase().as_str() {
                "OR" => Some((BinOp::Or, 1, false)),
                "XOR" => Some((BinOp::Xor, 2, false)),
                "AND" => Some((BinOp::And, 3, false)),
                "MOD" => Some((BinOp::Mod, 7, false)),
                _ => None,
            };
        }
        match token.text.as_str() {
            "&" => Some((BinOp::And, 3, false)),
            "=" => Some((BinOp::Eq, 4, false)),
            "<>" => Some((BinOp::Ne, 4, false)),
            "<" => Some((BinOp::Lt, 5, false)),
            "<=" => Some((BinOp::Le, 5, false)),
            ">" => Some((BinOp::Gt, 5, false)),
            ">=" => Some((BinOp::Ge, 5, false)),
            "+" => Some((BinOp::Add, 6, false)),
            "-" => Some((BinOp::Sub, 6, false)),
            "*" => Some((BinOp::Mul, 7, false)),
            "/" => Some((BinOp::Div, 7, false)),
            "**" => Some((BinOp::Pow, 8, true)),
            _ => None,
        }
    }

    fn parse_unary(&mut self) -> Expr {
        if self.at_keyword("NOT") {
            self.pos += 1;
            return Expr::Unary(UnOp::Not, Box::new(self.parse_unary()));
        }
        if self.at_text("-") {
            self.pos += 1;
            return Expr::Unary(UnOp::Neg, Box::new(self.parse_unary()));
        }
        if self.at_text("+") {
            self.pos += 1;
            return self.parse_unary();
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Expr {
        let mut expr = self.parse_primary();
        while self.at_text(".") {
            self.pos += 1;
            let Some(member) = self.advance() else { break };
            expr = Expr::Member(Box::new(expr), member.text.clone());
        }
        expr
    }

    fn parse_primary(&mut self) -> Expr {
        let Some(token) = self.peek() else {
            return Expr::Lit(Value::Unknown);
        };

        if token.text == "(" {
            self.pos += 1;
            let inner = self.parse_expr();
            self.eat_text(")");
            return inner;
        }

        match token.kind {
            TokenKind::NumberLiteral => {
                let value = Value::parse_literal(&token.text).unwrap_or(Value::Unknown);
                self.pos += 1;
                Expr::Lit(value)
            }
            TokenKind::StringLiteral => {
                let value = parse_string_literal(&token.text);
                self.pos += 1;
                Expr::Lit(Value::Str(value))
            }
            TokenKind::Identifier => {
                let name = token.text.clone();
                self.pos += 1;
                if self.at_text("(") {
                    let args = self.parse_simple_args();
                    return Expr::Call(name, args);
                }
                // Predefined boolean literals lex as identifiers.
                match Value::parse_literal(&name) {
                    Some(value @ Value::Bool(_)) => Expr::Lit(value),
                    _ => Expr::Var(name),
                }
            }
            _ => {
                self.pos += 1;
                Expr::Lit(Value::Unknown)
            }
        }
    }

    /// Parse `( expr, expr, … )` positional arguments for a function call.
    fn parse_simple_args(&mut self) -> Vec<Expr> {
        let mut args = Vec::new();
        self.eat_text("(");
        if self.eat_text(")") {
            return args;
        }
        loop {
            args.push(self.parse_expr());
            if self.eat_text(",") {
                continue;
            }
            break;
        }
        self.eat_text(")");
        args
    }

    /// Parse `( name := expr , expr , … )` for a function-block call, where each
    /// argument may be named or positional.
    fn parse_call_args(&mut self) -> Vec<CallArg> {
        let mut args = Vec::new();
        self.eat_text("(");
        if self.eat_text(")") {
            return args;
        }
        loop {
            let name = match (self.peek(), self.tokens.get(self.pos + 1)) {
                (Some(ident), Some(assign))
                    if ident.kind == TokenKind::Identifier && assign.text == ":=" =>
                {
                    let name = ident.text.clone();
                    self.pos += 2;
                    Some(name)
                }
                _ => None,
            };
            let value = self.parse_expr();
            args.push(CallArg { name, value });
            if self.eat_text(",") {
                continue;
            }
            break;
        }
        self.eat_text(")");
        args
    }
}

/// Strip the surrounding quotes of a string/wstring literal token. IEC `$`
/// escapes beyond the common quote/control forms are left as-is (the MVP source
/// set does not exercise them).
fn parse_string_literal(text: &str) -> String {
    let bytes = text.as_bytes();
    if bytes.len() < 2 {
        return String::new();
    }
    let quote = bytes[0] as char;
    if (quote == '\'' || quote == '"') && text.ends_with(quote) {
        return text[1..text.len() - 1].to_owned();
    }
    text.to_owned()
}

// ---------------------------------------------------------------------------
// Function-block instances
// ---------------------------------------------------------------------------

/// A live standard function-block instance held across scan cycles.
#[derive(Debug, Clone)]
pub(crate) enum FbInstance {
    Ton(Ton),
    Tof(Tof),
    Tp(Tp),
    Ctu(Ctu),
    Ctd(Ctd),
    Ctud(Ctud),
    RTrig(RTrig),
    FTrig(FTrig),
}

impl FbInstance {
    /// Create an instance for a standard function-block type, if recognized.
    pub(crate) fn new(type_name: &str) -> Option<Self> {
        Some(match type_name.to_ascii_uppercase().as_str() {
            "TON" => FbInstance::Ton(Ton::new(0)),
            "TOF" => FbInstance::Tof(Tof::new(0)),
            "TP" => FbInstance::Tp(Tp::new(0)),
            "CTU" => FbInstance::Ctu(Ctu::new()),
            "CTD" => FbInstance::Ctd(Ctd::new()),
            "CTUD" => FbInstance::Ctud(Ctud::new()),
            "R_TRIG" => FbInstance::RTrig(RTrig::new()),
            "F_TRIG" => FbInstance::FTrig(FTrig::new()),
            _ => return None,
        })
    }

    fn call(&mut self, args: &[(Option<String>, Value)], now_ms: i64) {
        match self {
            FbInstance::Ton(ton) => {
                ton.set_pt_ms(arg_i64(args, &["PT"], 1));
                ton.update(arg_bool(args, &["IN"], 0), now_ms);
            }
            FbInstance::Tof(tof) => {
                tof.set_pt_ms(arg_i64(args, &["PT"], 1));
                tof.update(arg_bool(args, &["IN"], 0), now_ms);
            }
            FbInstance::Tp(tp) => {
                tp.set_pt_ms(arg_i64(args, &["PT"], 1));
                tp.update(arg_bool(args, &["IN"], 0), now_ms);
            }
            FbInstance::Ctu(ctu) => {
                ctu.update(
                    arg_bool(args, &["CU"], 0),
                    arg_bool(args, &["RESET", "R"], 1),
                    arg_i64(args, &["PV"], 2),
                );
            }
            FbInstance::Ctd(ctd) => {
                ctd.update(
                    arg_bool(args, &["CD"], 0),
                    arg_bool(args, &["LOAD", "LD"], 1),
                    arg_i64(args, &["PV"], 2),
                );
            }
            FbInstance::Ctud(ctud) => {
                ctud.update(
                    arg_bool(args, &["CU"], 0),
                    arg_bool(args, &["CD"], 1),
                    arg_bool(args, &["RESET", "R"], 2),
                    arg_bool(args, &["LOAD", "LD"], 3),
                    arg_i64(args, &["PV"], 4),
                );
            }
            FbInstance::RTrig(rtrig) => {
                rtrig.update(arg_bool(args, &["CLK"], 0));
            }
            FbInstance::FTrig(ftrig) => {
                ftrig.update(arg_bool(args, &["CLK"], 0));
            }
        }
    }

    fn read_member(&self, member: &str) -> Value {
        let member = member.to_ascii_uppercase();
        match self {
            FbInstance::Ton(t) => timer_member(&member, t.q(), t.et_ms()),
            FbInstance::Tof(t) => timer_member(&member, t.q(), t.et_ms()),
            FbInstance::Tp(t) => timer_member(&member, t.q(), t.et_ms()),
            FbInstance::Ctu(c) => match member.as_str() {
                "Q" => Value::Bool(c.q()),
                "CV" => Value::Int(c.cv()),
                _ => Value::Unknown,
            },
            FbInstance::Ctd(c) => match member.as_str() {
                "Q" => Value::Bool(c.q()),
                "CV" => Value::Int(c.cv()),
                _ => Value::Unknown,
            },
            FbInstance::Ctud(c) => match member.as_str() {
                "QU" => Value::Bool(c.qu()),
                "QD" => Value::Bool(c.qd()),
                "CV" => Value::Int(c.cv()),
                _ => Value::Unknown,
            },
            FbInstance::RTrig(e) => match member.as_str() {
                "Q" => Value::Bool(e.q()),
                _ => Value::Unknown,
            },
            FbInstance::FTrig(e) => match member.as_str() {
                "Q" => Value::Bool(e.q()),
                _ => Value::Unknown,
            },
        }
    }

    /// The instance's output members in declaration order, for the debugger's
    /// variables view (reusing the same readouts as [`read_member`]).
    pub(crate) fn members(&self) -> Vec<(String, Value)> {
        let names: &[&str] = match self {
            FbInstance::Ton(_) | FbInstance::Tof(_) | FbInstance::Tp(_) => &["Q", "ET"],
            FbInstance::Ctu(_) | FbInstance::Ctd(_) => &["Q", "CV"],
            FbInstance::Ctud(_) => &["QU", "QD", "CV"],
            FbInstance::RTrig(_) | FbInstance::FTrig(_) => &["Q"],
        };
        names
            .iter()
            .map(|member| ((*member).to_owned(), self.read_member(member)))
            .collect()
    }
}

fn timer_member(member: &str, q: bool, et_ms: i64) -> Value {
    match member {
        "Q" => Value::Bool(q),
        "ET" => Value::Time(et_ms),
        _ => Value::Unknown,
    }
}

fn arg_value<'a>(
    args: &'a [(Option<String>, Value)],
    names: &[&str],
    position: usize,
) -> Option<&'a Value> {
    if let Some((_, value)) = args.iter().find(|(name, _)| {
        name.as_deref().is_some_and(|name| {
            names
                .iter()
                .any(|candidate| name.eq_ignore_ascii_case(candidate))
        })
    }) {
        return Some(value);
    }
    // Fall back to positional only when no argument in the call is named.
    if args.iter().all(|(name, _)| name.is_none()) {
        return args.get(position).map(|(_, value)| value);
    }
    None
}

fn arg_bool(args: &[(Option<String>, Value)], names: &[&str], position: usize) -> bool {
    arg_value(args, names, position).is_some_and(as_bool)
}

fn arg_i64(args: &[(Option<String>, Value)], names: &[&str], position: usize) -> i64 {
    arg_value(args, names, position).map(as_i64).unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Execution
// ---------------------------------------------------------------------------

/// What the interpreter should do after a [`DebugHook`] inspects a statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StepAction {
    /// Keep executing (the hook may have blocked to honor a step/breakpoint).
    Continue,
    /// Abort the rest of the scan (debugger disconnected / session stopped).
    Stop,
}

/// Hook the interpreter calls before executing each statement, enabling the
/// stepping debugger to pause, inspect live state, and resume.
pub(crate) trait DebugHook {
    /// Called immediately before statement at `line` (1-based) runs, at block
    /// nesting `depth`, with read-only access to live variable and FB state.
    fn at_statement(
        &mut self,
        line: u32,
        depth: u32,
        vars: &VariableTable,
        fbs: &HashMap<String, FbInstance>,
    ) -> StepAction;

    /// Called once at the start of each scan's logic phase, before any
    /// statement runs. Default: ignore.
    fn enter_scan(&mut self, _scan: u64) {}

    /// Whether the session has been stopped (so the runtime can end early).
    /// Default: never stopped.
    fn is_stopped(&self) -> bool {
        false
    }
}

/// Mutable execution context borrowed from the runtime for one logic scan.
pub(crate) struct ExecState<'a> {
    pub(crate) vars: &'a mut VariableTable,
    pub(crate) fbs: &'a mut HashMap<String, FbInstance>,
    pub(crate) now_ms: i64,
    /// Optional stepping-debug hook; `None` for normal (non-debug) execution.
    pub(crate) hook: Option<&'a mut dyn DebugHook>,
    /// Block nesting level of the statements currently running (0 = top level).
    pub(crate) depth: u32,
}

/// Non-local control flow signalled out of a statement block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flow {
    Normal,
    Continue,
    Exit,
    Return,
}

/// Safety bound on `WHILE`/`REPEAT` iterations so a non-terminating loop in a
/// development program cannot hang the runtime.
const LOOP_GUARD: u64 = 1_000_000;

/// Execute a statement block, returning how it terminated.
pub(crate) fn exec_block(stmts: &[Stmt], state: &mut ExecState) -> bool {
    matches!(run_block(stmts, state), Flow::Return)
}

fn run_block(stmts: &[Stmt], state: &mut ExecState) -> Flow {
    // Each nested block is one level deeper, so Step Over/Out can compare
    // nesting levels. The top-level body enters via `exec_block` -> here too.
    state.depth += 1;
    let mut flow = Flow::Normal;
    for stmt in stmts {
        flow = run_stmt(stmt, state);
        if flow != Flow::Normal {
            break;
        }
    }
    state.depth -= 1;
    flow
}

fn run_stmt(stmt: &Stmt, state: &mut ExecState) -> Flow {
    // Stepping-debug control: let the hook pause and inspect live state before
    // this statement runs. Take the hook out so the snapshot can borrow
    // `vars`/`fbs` alongside it, then restore it.
    if state.hook.is_some() {
        let hook = state.hook.take().expect("hook present");
        let action = hook.at_statement(stmt.line, state.depth, state.vars, state.fbs);
        state.hook = Some(hook);
        if matches!(action, StepAction::Stop) {
            return Flow::Return;
        }
    }
    match &stmt.kind {
        StmtKind::Assign { target, value } => {
            let evaluated = eval(value, state);
            state.vars.set(target, evaluated);
            Flow::Normal
        }
        StmtKind::FbCall { instance, args } => {
            let evaluated: Vec<(Option<String>, Value)> = args
                .iter()
                .map(|arg| (arg.name.clone(), eval(&arg.value, state)))
                .collect();
            let key = instance.to_ascii_lowercase();
            if let Some(fb) = state.fbs.get_mut(&key) {
                fb.call(&evaluated, state.now_ms);
            }
            Flow::Normal
        }
        StmtKind::If {
            branches,
            else_body,
        } => {
            for (cond, body) in branches {
                if as_bool(&eval(cond, state)) {
                    return run_block(body, state);
                }
            }
            run_block(else_body, state)
        }
        StmtKind::Case {
            selector,
            branches,
            else_body,
        } => {
            let value = as_i64(&eval(selector, state));
            for (labels, body) in branches {
                if labels.iter().any(|label| label_matches(label, value)) {
                    return run_block(body, state);
                }
            }
            run_block(else_body, state)
        }
        StmtKind::For {
            var,
            from,
            to,
            by,
            body,
        } => run_for(var, from, to, by.as_ref(), body, state),
        StmtKind::While { cond, body } => {
            let mut guard = 0u64;
            while as_bool(&eval(cond, state)) {
                match run_block(body, state) {
                    Flow::Exit => break,
                    Flow::Return => return Flow::Return,
                    _ => {}
                }
                guard += 1;
                if guard >= LOOP_GUARD {
                    break;
                }
            }
            Flow::Normal
        }
        StmtKind::Repeat { body, until } => {
            let mut guard = 0u64;
            loop {
                match run_block(body, state) {
                    Flow::Exit => break,
                    Flow::Return => return Flow::Return,
                    _ => {}
                }
                if as_bool(&eval(until, state)) {
                    break;
                }
                guard += 1;
                if guard >= LOOP_GUARD {
                    break;
                }
            }
            Flow::Normal
        }
        StmtKind::Return => Flow::Return,
        StmtKind::Exit => Flow::Exit,
        StmtKind::Continue => Flow::Continue,
    }
}

fn run_for(
    var: &str,
    from: &Expr,
    to: &Expr,
    by: Option<&Expr>,
    body: &[Stmt],
    state: &mut ExecState,
) -> Flow {
    let start = as_i64(&eval(from, state));
    let end = as_i64(&eval(to, state));
    let step = by.map(|expr| as_i64(&eval(expr, state))).unwrap_or(1);
    if step == 0 {
        return Flow::Normal;
    }

    let mut index = start;
    loop {
        if (step > 0 && index > end) || (step < 0 && index < end) {
            break;
        }
        state.vars.set(var, Value::Int(index));
        match run_block(body, state) {
            Flow::Exit => break,
            Flow::Return => return Flow::Return,
            _ => {}
        }
        index += step;
    }
    Flow::Normal
}

fn label_matches(label: &CaseLabel, value: i64) -> bool {
    match label {
        CaseLabel::Single(target) => *target == value,
        CaseLabel::Range(low, high) => *low <= value && value <= *high,
    }
}

fn eval(expr: &Expr, state: &ExecState) -> Value {
    match expr {
        Expr::Lit(value) => value.clone(),
        Expr::Var(name) => state.vars.get(name).cloned().unwrap_or(Value::Unknown),
        Expr::Member(base, member) => {
            if let Expr::Var(name) = base.as_ref()
                && let Some(fb) = state.fbs.get(&name.to_ascii_lowercase())
            {
                return fb.read_member(member);
            }
            Value::Unknown
        }
        Expr::Unary(op, operand) => eval_unary(*op, &eval(operand, state)),
        Expr::Binary(op, lhs, rhs) => eval_binary(*op, &eval(lhs, state), &eval(rhs, state)),
        Expr::Call(name, args) => {
            let evaluated: Vec<Value> = args.iter().map(|arg| eval(arg, state)).collect();
            stdlib::call(name, &evaluated)
        }
    }
}

fn eval_unary(op: UnOp, value: &Value) -> Value {
    match (op, value) {
        (UnOp::Not, Value::Bool(flag)) => Value::Bool(!flag),
        (UnOp::Not, Value::Int(bits)) => Value::Int(!bits),
        (UnOp::Neg, Value::Int(int)) => Value::Int(-int),
        (UnOp::Neg, Value::Real(real)) => Value::Real(-real),
        _ => Value::Unknown,
    }
}

fn eval_binary(op: BinOp, left: &Value, right: &Value) -> Value {
    match op {
        BinOp::Add => Value::add(left.clone(), right.clone()),
        BinOp::Sub => Value::sub(left.clone(), right.clone()),
        BinOp::Mul => arithmetic(left, right, |a, b| a * b, |a, b| a * b),
        BinOp::Div => divide(left, right),
        BinOp::Mod => match (left, right) {
            (Value::Int(a), Value::Int(b)) if *b != 0 => Value::Int(a % b),
            _ => Value::Unknown,
        },
        BinOp::Pow => match (to_f64(left), to_f64(right)) {
            (Some(base), Some(exp)) => Value::Real(base.powf(exp)),
            _ => Value::Unknown,
        },
        BinOp::And => logical_or_bitwise(left, right, |a, b| a && b, |a, b| a & b),
        BinOp::Or => logical_or_bitwise(left, right, |a, b| a || b, |a, b| a | b),
        BinOp::Xor => logical_or_bitwise(left, right, |a, b| a ^ b, |a, b| a ^ b),
        BinOp::Eq => compare(left, right, |ord| ord == std::cmp::Ordering::Equal),
        BinOp::Ne => compare(left, right, |ord| ord != std::cmp::Ordering::Equal),
        BinOp::Lt => compare(left, right, |ord| ord == std::cmp::Ordering::Less),
        BinOp::Le => compare(left, right, |ord| ord != std::cmp::Ordering::Greater),
        BinOp::Gt => compare(left, right, |ord| ord == std::cmp::Ordering::Greater),
        BinOp::Ge => compare(left, right, |ord| ord != std::cmp::Ordering::Less),
    }
}

fn arithmetic(
    left: &Value,
    right: &Value,
    int_op: fn(i64, i64) -> i64,
    real_op: fn(f64, f64) -> f64,
) -> Value {
    match (left, right) {
        (Value::Int(a), Value::Int(b)) => Value::Int(int_op(*a, *b)),
        (Value::Real(a), Value::Real(b)) => Value::Real(real_op(*a, *b)),
        (Value::Real(a), Value::Int(b)) => Value::Real(real_op(*a, *b as f64)),
        (Value::Int(a), Value::Real(b)) => Value::Real(real_op(*a as f64, *b)),
        _ => Value::Unknown,
    }
}

fn divide(left: &Value, right: &Value) -> Value {
    match (left, right) {
        (Value::Int(_), Value::Int(0)) => Value::Unknown,
        (Value::Int(a), Value::Int(b)) => Value::Int(a / b),
        _ => match (to_f64(left), to_f64(right)) {
            (Some(a), Some(b)) if b != 0.0 => Value::Real(a / b),
            _ => Value::Unknown,
        },
    }
}

fn logical_or_bitwise(
    left: &Value,
    right: &Value,
    bool_op: fn(bool, bool) -> bool,
    int_op: fn(i64, i64) -> i64,
) -> Value {
    match (left, right) {
        (Value::Bool(a), Value::Bool(b)) => Value::Bool(bool_op(*a, *b)),
        (Value::Int(a), Value::Int(b)) => Value::Int(int_op(*a, *b)),
        _ => Value::Unknown,
    }
}

fn compare(left: &Value, right: &Value, pick: fn(std::cmp::Ordering) -> bool) -> Value {
    let ordering = match (left, right) {
        (Value::Int(a), Value::Int(b)) => a.partial_cmp(b),
        (Value::Time(a), Value::Time(b)) => a.partial_cmp(b),
        (Value::Str(a), Value::Str(b)) => a.partial_cmp(b),
        (Value::Bool(a), Value::Bool(b)) => a.partial_cmp(b),
        _ => match (to_f64(left), to_f64(right)) {
            (Some(a), Some(b)) => a.partial_cmp(&b),
            _ => None,
        },
    };
    match ordering {
        Some(ordering) => Value::Bool(pick(ordering)),
        None => Value::Unknown,
    }
}

fn to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Real(real) => Some(*real),
        Value::Int(int) => Some(*int as f64),
        _ => None,
    }
}

fn as_i64(value: &Value) -> i64 {
    match value {
        Value::Int(int) => *int,
        Value::Real(real) => *real as i64,
        Value::Time(ms) => *ms,
        Value::Bool(flag) => *flag as i64,
        _ => 0,
    }
}

fn as_bool(value: &Value) -> bool {
    match value {
        Value::Bool(flag) => *flag,
        Value::Int(int) => *int != 0,
        _ => false,
    }
}
