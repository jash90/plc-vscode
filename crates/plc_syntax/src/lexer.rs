use crate::{SyntaxDiagnostic, TextRange};

/// Structured Text token categories. Trivia is intentionally preserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
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

/// Source-preserving token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub range: TextRange,
    pub text: String,
}

impl Token {
    fn new(kind: TokenKind, start: usize, end: usize, source: &str) -> Self {
        Self {
            kind,
            range: TextRange::new(start, end),
            text: source[start..end].to_owned(),
        }
    }

    pub fn is_trivia(&self) -> bool {
        matches!(
            self.kind,
            TokenKind::Whitespace | TokenKind::Newline | TokenKind::Comment
        )
    }

    pub fn keyword_eq(&self, keyword: &str) -> bool {
        self.kind == TokenKind::Keyword && self.text.eq_ignore_ascii_case(keyword)
    }
}

/// Lexer output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexedSource {
    tokens: Vec<Token>,
    diagnostics: Vec<SyntaxDiagnostic>,
}

impl LexedSource {
    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    pub fn diagnostics(&self) -> &[SyntaxDiagnostic] {
        &self.diagnostics
    }
}

/// Lex a Structured Text source file while preserving comments and whitespace.
pub fn lex_source(source: &str) -> LexedSource {
    let mut tokens = Vec::new();
    let mut diagnostics = Vec::new();
    let bytes = source.as_bytes();
    let mut cursor = 0usize;

    while cursor < bytes.len() {
        let start = cursor;
        let ch = source[cursor..]
            .chars()
            .next()
            .expect("cursor always points inside source");

        match ch {
            '\r' => {
                cursor += ch.len_utf8();
                if source[cursor..].starts_with('\n') {
                    cursor += 1;
                }
                tokens.push(Token::new(TokenKind::Newline, start, cursor, source));
            }
            '\n' => {
                cursor += 1;
                tokens.push(Token::new(TokenKind::Newline, start, cursor, source));
            }
            ' ' | '\t' | '\u{000C}' => {
                cursor += ch.len_utf8();
                while cursor < bytes.len() {
                    let next = source[cursor..].chars().next().unwrap();
                    if matches!(next, ' ' | '\t' | '\u{000C}') {
                        cursor += next.len_utf8();
                    } else {
                        break;
                    }
                }
                tokens.push(Token::new(TokenKind::Whitespace, start, cursor, source));
            }
            '(' if source[cursor..].starts_with("(*") => {
                let (end, closed) = scan_block_comment(source, cursor);
                cursor = end;
                tokens.push(Token::new(TokenKind::Comment, start, cursor, source));
                if !closed {
                    diagnostics.push(SyntaxDiagnostic {
                        code: "PLC0001",
                        range: TextRange::new(start, cursor),
                        message: "Unclosed block comment: expected closing *)".to_owned(),
                    });
                }
            }
            '/' if source[cursor..].starts_with("//") => {
                cursor += 2;
                while cursor < bytes.len() {
                    let next = source[cursor..].chars().next().unwrap();
                    if matches!(next, '\r' | '\n') {
                        break;
                    }
                    cursor += next.len_utf8();
                }
                tokens.push(Token::new(TokenKind::Comment, start, cursor, source));
            }
            // Single-quoted STRING and double-quoted WSTRING literals share the
            // same scanning rules (IEC `$` escapes); the closing quote matches
            // the opener.
            '\'' | '"' => {
                let (end, closed) = scan_string_literal(source, cursor, ch);
                cursor = end;
                tokens.push(Token::new(TokenKind::StringLiteral, start, cursor, source));
                if !closed {
                    diagnostics.push(SyntaxDiagnostic {
                        code: "SYN0001",
                        range: TextRange::new(start, cursor),
                        message: format!("Unclosed string literal: expected closing {ch}"),
                    });
                }
            }
            c if is_identifier_start(c) => {
                cursor += c.len_utf8();
                while cursor < bytes.len() {
                    let next = source[cursor..].chars().next().unwrap();
                    if is_identifier_continue(next) {
                        cursor += next.len_utf8();
                    } else {
                        break;
                    }
                }
                // A `#` after an identifier starts an IEC typed/duration literal
                // (e.g. `T#20ms`, `BYTE#9`, `INT#16#FF`); lex it as one literal.
                if source[cursor..].starts_with('#') {
                    cursor = scan_typed_literal_body(source, cursor);
                    tokens.push(Token::new(TokenKind::NumberLiteral, start, cursor, source));
                } else {
                    let kind = if is_keyword(&source[start..cursor]) {
                        TokenKind::Keyword
                    } else {
                        TokenKind::Identifier
                    };
                    tokens.push(Token::new(kind, start, cursor, source));
                }
            }
            c if c.is_ascii_digit() => {
                cursor += c.len_utf8();
                while cursor < bytes.len() {
                    let next = source[cursor..].chars().next().unwrap();
                    if next.is_ascii_alphanumeric() || matches!(next, '_' | '.') {
                        cursor += next.len_utf8();
                    } else {
                        break;
                    }
                }
                // A `#` after a number starts an IEC base/radix literal (`16#FF`,
                // `2#1010_0110`); absorb it into the same literal token.
                if source[cursor..].starts_with('#') {
                    cursor = scan_typed_literal_body(source, cursor);
                }
                tokens.push(Token::new(TokenKind::NumberLiteral, start, cursor, source));
            }
            '{' => {
                // Vendor pragma / brace-delimited metadata (`{attribute 'hide'}`,
                // `{region}`). Consume through the matching `}` and treat it as
                // trivia so it does not cascade into the parser.
                cursor += '{'.len_utf8();
                while cursor < bytes.len() {
                    let next = source[cursor..].chars().next().unwrap();
                    cursor += next.len_utf8();
                    if next == '}' {
                        break;
                    }
                }
                tokens.push(Token::new(TokenKind::Comment, start, cursor, source));
            }
            '%' => {
                // IEC located variable / direct address (`%IX0.0`, `%MW10`,
                // `%QB4`). Lex the whole address as one operand token; semantic
                // validation of the location is deferred.
                cursor += '%'.len_utf8();
                while cursor < bytes.len() {
                    let next = source[cursor..].chars().next().unwrap();
                    // Address chars plus hierarchical `.`, but never a `..` range.
                    let is_address_char = next.is_ascii_alphanumeric()
                        || next == '_'
                        || (next == '.' && !source[cursor + next.len_utf8()..].starts_with('.'));
                    if !is_address_char {
                        break;
                    }
                    cursor += next.len_utf8();
                }
                tokens.push(Token::new(TokenKind::Identifier, start, cursor, source));
            }
            _ if scan_operator(source, cursor).is_some() => {
                cursor = scan_operator(source, cursor).unwrap();
                tokens.push(Token::new(TokenKind::Operator, start, cursor, source));
            }
            _ => {
                cursor += ch.len_utf8();
                tokens.push(Token::new(TokenKind::Invalid, start, cursor, source));
                diagnostics.push(SyntaxDiagnostic {
                    code: "SYN0000",
                    range: TextRange::new(start, cursor),
                    message: format!("Invalid token `{ch}`"),
                });
            }
        }
    }

    LexedSource {
        tokens,
        diagnostics,
    }
}

fn scan_block_comment(source: &str, start: usize) -> (usize, bool) {
    let mut cursor = start + 2;
    let mut depth = 1usize;

    while cursor < source.len() {
        if source[cursor..].starts_with("(*") {
            depth += 1;
            cursor += 2;
        } else if source[cursor..].starts_with("*)") {
            depth -= 1;
            cursor += 2;
            if depth == 0 {
                return (cursor, true);
            }
        } else {
            let ch = source[cursor..].chars().next().unwrap();
            cursor += ch.len_utf8();
        }
    }

    (cursor, false)
}

/// Scan a string literal delimited by `quote` (`'` for STRING, `"` for
/// WSTRING). Handles IEC `$` escapes (`$'`, `$"`, `$$`, `$N`, `$0048`, …) so an
/// escaped quote does not end the literal, plus the legacy doubled-quote form.
/// Returns the end offset and whether the literal was closed.
fn scan_string_literal(source: &str, start: usize, quote: char) -> (usize, bool) {
    let mut cursor = start + quote.len_utf8();

    while cursor < source.len() {
        let ch = source[cursor..].chars().next().unwrap();
        cursor += ch.len_utf8();
        if ch == '$' {
            // The next character is escaped (control char, quote, or the first
            // digit of a `$<hex>` code); consume it so it cannot terminate.
            if let Some(next) = source[cursor..].chars().next() {
                cursor += next.len_utf8();
            }
            continue;
        }
        if ch == quote {
            if source[cursor..].starts_with(quote) {
                cursor += quote.len_utf8();
            } else {
                return (cursor, true);
            }
        }
        if matches!(ch, '\r' | '\n') {
            return (cursor, false);
        }
    }

    (cursor, false)
}

/// Scan the body of an IEC typed/base/duration literal starting at the `#`
/// separator (the `#FF` of `16#FF`, the `#20ms` of `T#20ms`). Consumes the `#`,
/// an optional sign for signed durations, then literal characters. Stops at
/// whitespace, operators, or a `..` range so CASE ranges keep their separator.
fn scan_typed_literal_body(source: &str, start: usize) -> usize {
    let mut cursor = start + '#'.len_utf8();

    if let Some(sign) = source[cursor..].chars().next()
        && (sign == '+' || sign == '-')
    {
        cursor += sign.len_utf8();
    }

    while cursor < source.len() {
        let ch = source[cursor..].chars().next().unwrap();
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '#' {
            cursor += ch.len_utf8();
        } else if ch == '.' && !source[cursor + ch.len_utf8()..].starts_with('.') {
            // A lone `.` is a decimal point; `..` is a range operator — stop.
            cursor += ch.len_utf8();
        } else {
            break;
        }
    }

    cursor
}

fn scan_operator(source: &str, cursor: usize) -> Option<usize> {
    for op in [":=", "<=", ">=", "<>", "..", "=>"] {
        if source[cursor..].starts_with(op) {
            return Some(cursor + op.len());
        }
    }

    let ch = source[cursor..].chars().next()?;
    if matches!(
        ch,
        ':' | ';'
            | ','
            | '.'
            | '('
            | ')'
            | '['
            | ']'
            | '+'
            | '-'
            | '*'
            | '/'
            | '='
            | '<'
            | '>'
            | '&'
            | '^'
    ) {
        Some(cursor + ch.len_utf8())
    } else {
        None
    }
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn is_keyword(text: &str) -> bool {
    matches!(
        text.to_ascii_uppercase().as_str(),
        "ACTION"
            | "ARRAY"
            | "BY"
            | "CASE"
            | "CONSTANT"
            | "CONTINUE"
            | "DO"
            | "ELSE"
            | "ELSIF"
            | "END_ACTION"
            | "END_CASE"
            | "END_FOR"
            | "END_FUNCTION"
            | "END_FUNCTION_BLOCK"
            | "END_IF"
            | "END_PROGRAM"
            | "END_REPEAT"
            | "END_STRUCT"
            | "END_TYPE"
            | "END_VAR"
            | "END_WHILE"
            | "EXIT"
            | "FOR"
            | "FUNCTION"
            | "FUNCTION_BLOCK"
            | "IF"
            | "OF"
            | "PROGRAM"
            | "REPEAT"
            | "RETURN"
            | "STRUCT"
            | "THEN"
            | "TO"
            | "TYPE"
            | "UNTIL"
            | "VAR"
            | "VAR_GLOBAL"
            | "VAR_IN_OUT"
            | "VAR_INPUT"
            | "VAR_OUTPUT"
            | "VAR_TEMP"
            | "WHILE"
    )
}
