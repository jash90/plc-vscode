//! Structured Text syntax primitives for PLC VS Code.
//!
//! The crate owns source-preserving lexical analysis that downstream crates
//! consume instead of duplicating syntax checks in CLI, LSP, or compiler-core
//! consumers. A rowan-backed concrete syntax tree preserves trivia and exact
//! token text; the parser facade is added by a later task.

mod lexer;

pub mod cst;

pub use lexer::{LexedSource, Token, TokenKind, lex_source};

/// Byte-based half-open source range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextRange {
    pub start: usize,
    pub end: usize,
}

impl TextRange {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn is_empty(self) -> bool {
        self.start == self.end
    }
}

/// Recoverable syntax diagnostic produced by lexing or parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxDiagnostic {
    pub code: &'static str,
    pub range: TextRange,
    pub message: String,
}
