use rowan::{GreenNodeBuilder, Language, SyntaxKind as RowanSyntaxKind};

use crate::{TokenKind, lex_source};

/// Rowan syntax kinds used by the first CST layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum SyntaxKind {
    Root = 0,
    Keyword,
    Identifier,
    NumberLiteral,
    StringLiteral,
    Operator,
    Comment,
    Whitespace,
    Newline,
    Invalid,
}

impl From<SyntaxKind> for RowanSyntaxKind {
    fn from(kind: SyntaxKind) -> Self {
        Self(kind as u16)
    }
}

impl From<TokenKind> for SyntaxKind {
    fn from(kind: TokenKind) -> Self {
        match kind {
            TokenKind::Keyword => Self::Keyword,
            TokenKind::Identifier => Self::Identifier,
            TokenKind::NumberLiteral => Self::NumberLiteral,
            TokenKind::StringLiteral => Self::StringLiteral,
            TokenKind::Operator => Self::Operator,
            TokenKind::Comment => Self::Comment,
            TokenKind::Whitespace => Self::Whitespace,
            TokenKind::Newline => Self::Newline,
            TokenKind::Invalid => Self::Invalid,
        }
    }
}

/// Rowan language marker for Structured Text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlcLanguage {}

impl Language for PlcLanguage {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: RowanSyntaxKind) -> Self::Kind {
        match raw.0 {
            0 => SyntaxKind::Root,
            1 => SyntaxKind::Keyword,
            2 => SyntaxKind::Identifier,
            3 => SyntaxKind::NumberLiteral,
            4 => SyntaxKind::StringLiteral,
            5 => SyntaxKind::Operator,
            6 => SyntaxKind::Comment,
            7 => SyntaxKind::Whitespace,
            8 => SyntaxKind::Newline,
            9 => SyntaxKind::Invalid,
            _ => SyntaxKind::Invalid,
        }
    }

    fn kind_to_raw(kind: Self::Kind) -> RowanSyntaxKind {
        kind.into()
    }
}

pub type SyntaxNode = rowan::SyntaxNode<PlcLanguage>;
pub type SyntaxToken = rowan::SyntaxToken<PlcLanguage>;

/// Source-preserving concrete syntax tree.
#[derive(Debug, Clone)]
pub struct ConcreteSyntaxTree {
    root: SyntaxNode,
}

impl ConcreteSyntaxTree {
    pub fn root(&self) -> SyntaxNode {
        self.root.clone()
    }

    pub fn tokens(&self) -> impl Iterator<Item = SyntaxToken> {
        self.root
            .descendants_with_tokens()
            .filter_map(|element| element.into_token())
    }
}

/// Build a rowan-backed CST that preserves all lexer tokens, including trivia.
pub fn build_cst(source: &str) -> ConcreteSyntaxTree {
    let lexed = lex_source(source);
    let mut builder = GreenNodeBuilder::new();

    builder.start_node(SyntaxKind::Root.into());
    for token in lexed.tokens() {
        builder.token(SyntaxKind::from(token.kind).into(), token.text.as_str());
    }
    builder.finish_node();

    ConcreteSyntaxTree {
        root: SyntaxNode::new_root(builder.finish()),
    }
}
