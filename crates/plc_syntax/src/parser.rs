use crate::{LexedSource, SyntaxDiagnostic, TextRange, Token, TokenKind, lex_source};

/// POU kind recognized by the parser facade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PouKind {
    Program,
    Function,
    FunctionBlock,
    Action,
}

impl PouKind {
    pub fn start_keyword(self) -> &'static str {
        match self {
            Self::Program => "PROGRAM",
            Self::Function => "FUNCTION",
            Self::FunctionBlock => "FUNCTION_BLOCK",
            Self::Action => "ACTION",
        }
    }

    pub fn end_keyword(self) -> &'static str {
        match self {
            Self::Program => "END_PROGRAM",
            Self::Function => "END_FUNCTION",
            Self::FunctionBlock => "END_FUNCTION_BLOCK",
            Self::Action => "END_ACTION",
        }
    }
}

/// Structured Text variable declaration block kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarBlockKind {
    Var,
    Input,
    Output,
    InOut,
    Global,
    Temp,
}

/// Parsed variable declaration summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariableDeclaration {
    pub name: String,
    pub type_name: String,
    pub initializer: Option<String>,
    pub range: TextRange,
}

/// Parsed declaration block summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclarationBlock {
    pub kind: VarBlockKind,
    pub declarations: Vec<VariableDeclaration>,
    pub range: TextRange,
}

/// MVP statement categories recognized for syntax consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatementKind {
    Assignment,
    If,
    Case,
    For,
    While,
    Repeat,
    Return,
    Exit,
    Continue,
}

/// Parsed statement summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    pub kind: StatementKind,
    pub range: TextRange,
    pub target: Option<String>,
    pub expression: Option<String>,
}

/// Parsed program organization unit summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pou {
    pub kind: PouKind,
    pub name: Option<String>,
    pub range: TextRange,
    pub declaration_blocks: Vec<DeclarationBlock>,
    pub statements: Vec<Statement>,
}

/// Parser facade output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxParse {
    lexed: LexedSource,
    units: Vec<Pou>,
    diagnostics: Vec<SyntaxDiagnostic>,
}

impl SyntaxParse {
    pub fn tokens(&self) -> &[Token] {
        self.lexed.tokens()
    }

    pub fn units(&self) -> &[Pou] {
        &self.units
    }

    pub fn diagnostics(&self) -> &[SyntaxDiagnostic] {
        &self.diagnostics
    }

    pub fn lex_diagnostics(&self) -> &[SyntaxDiagnostic] {
        self.lexed.diagnostics()
    }
}

/// Parse Structured Text source into a recoverable POU summary.
pub fn parse_source(source: &str) -> SyntaxParse {
    let lexed = lex_source(source);
    let mut diagnostics = lexed.diagnostics().to_vec();
    let significant: Vec<(usize, &Token)> = lexed
        .tokens()
        .iter()
        .enumerate()
        .filter(|(_, token)| !token.is_trivia())
        .collect();
    let mut units = Vec::new();
    let mut cursor = 0usize;

    while cursor < significant.len() {
        // IEC `CONFIGURATION … END_CONFIGURATION` wrappers (and the `PROGRAM
        // <instance> WITH …` mappings inside them) are not POUs. These words are
        // not lexer keywords, so match on text and skip the whole block to avoid
        // a spurious missing-terminator on the inner `PROGRAM` instance.
        if significant[cursor]
            .1
            .text
            .eq_ignore_ascii_case("CONFIGURATION")
        {
            let mut search = cursor + 1;
            while search < significant.len()
                && !significant[search]
                    .1
                    .text
                    .eq_ignore_ascii_case("END_CONFIGURATION")
            {
                search += 1;
            }
            cursor = if search < significant.len() {
                search + 1
            } else {
                significant.len()
            };
            continue;
        }

        let Some(kind) = pou_start_kind(significant[cursor].1) else {
            cursor += 1;
            continue;
        };

        let start_token = significant[cursor].1;
        let name = significant
            .get(cursor + 1)
            .map(|(_, token)| token)
            .filter(|token| token.kind == TokenKind::Identifier)
            .map(|token| token.text.clone());

        // CODESYS INTERFACE forward declarations (`FUNCTION Name;`) have no body
        // or terminator; treat them as non-POU and skip so they do not produce a
        // false missing-END_FUNCTION diagnostic. The real definition inside
        // IMPLEMENTATION still parses normally.
        let after_header = cursor + 1 + usize::from(name.is_some());
        if significant
            .get(after_header)
            .is_some_and(|(_, token)| token.text == ";")
        {
            cursor = after_header + 1;
            continue;
        }

        let mut search = cursor + 1;
        let mut end_index = None;
        let mut next_pou_index = None;

        while search < significant.len() {
            let token = significant[search].1;
            if token.keyword_eq(kind.end_keyword()) {
                end_index = Some(search);
                break;
            }
            if pou_start_kind(token).is_some() {
                next_pou_index = Some(search);
                break;
            }
            search += 1;
        }

        let body_end = end_index.unwrap_or_else(|| next_pou_index.unwrap_or(significant.len()));
        let body = &significant[cursor + 1..body_end];
        let declaration_blocks = parse_declaration_blocks(body);
        let statements = parse_statements(body);

        if let Some(end) = end_index {
            let end_token = significant[end].1;
            units.push(Pou {
                kind,
                name,
                range: TextRange::new(start_token.range.start, end_token.range.end),
                declaration_blocks,
                statements,
            });
            cursor = end + 1;
        } else {
            diagnostics.push(missing_end_diagnostic(kind, start_token.range));
            units.push(Pou {
                kind,
                name,
                range: TextRange::new(start_token.range.start, start_token.range.end),
                declaration_blocks,
                statements,
            });
            cursor = next_pou_index.unwrap_or(cursor + 1);
        }
    }

    SyntaxParse {
        lexed,
        units,
        diagnostics,
    }
}

fn parse_declaration_blocks(tokens: &[(usize, &Token)]) -> Vec<DeclarationBlock> {
    let mut blocks = Vec::new();
    let mut cursor = 0usize;

    while cursor < tokens.len() {
        let Some(kind) = var_block_kind(tokens[cursor].1) else {
            cursor += 1;
            continue;
        };

        let start = tokens[cursor].1.range.start;
        let mut end = tokens[cursor].1.range.end;
        let mut declarations = Vec::new();
        cursor += 1;

        while cursor < tokens.len() && !tokens[cursor].1.keyword_eq("END_VAR") {
            if let Some((declaration, next_cursor)) = parse_declaration(tokens, cursor) {
                end = declaration.range.end;
                declarations.push(declaration);
                cursor = next_cursor;
            } else {
                cursor += 1;
            }
        }

        if cursor < tokens.len() && tokens[cursor].1.keyword_eq("END_VAR") {
            end = tokens[cursor].1.range.end;
            cursor += 1;
        }

        blocks.push(DeclarationBlock {
            kind,
            declarations,
            range: TextRange::new(start, end),
        });
    }

    blocks
}

fn parse_declaration(
    tokens: &[(usize, &Token)],
    cursor: usize,
) -> Option<(VariableDeclaration, usize)> {
    let name = tokens.get(cursor)?.1;
    if name.kind != TokenKind::Identifier {
        return None;
    }

    // Optional `AT <located-address>` clause (`binvar AT %IX7.8 : BOOL`). The
    // address operand is skipped here; its token range is preserved for future
    // semantic validation.
    let mut colon_index = cursor + 1;
    if tokens
        .get(colon_index)
        .is_some_and(|(_, token)| token.text.eq_ignore_ascii_case("AT"))
    {
        colon_index += 2;
    }

    let colon = tokens.get(colon_index)?.1;
    if colon.text != ":" {
        return None;
    }
    let type_token = tokens.get(colon_index + 1)?.1;
    if !matches!(type_token.kind, TokenKind::Identifier | TokenKind::Keyword) {
        return None;
    }

    let mut next = colon_index + 2;
    let mut initializer = None;
    if tokens
        .get(next)
        .is_some_and(|(_, token)| token.text == ":=")
    {
        let init_start = next + 1;
        next = init_start;
        while next < tokens.len() && tokens[next].1.text != ";" {
            next += 1;
        }
        initializer = Some(join_token_text(&tokens[init_start..next]));
    } else {
        while next < tokens.len() && tokens[next].1.text != ";" {
            next += 1;
        }
    }

    let end = tokens
        .get(next)
        .map(|(_, token)| token.range.end)
        .unwrap_or(type_token.range.end);

    Some((
        VariableDeclaration {
            name: name.text.clone(),
            type_name: type_token.text.clone(),
            initializer,
            range: TextRange::new(name.range.start, end),
        },
        next.saturating_add(1),
    ))
}

fn parse_statements(tokens: &[(usize, &Token)]) -> Vec<Statement> {
    let mut statements = Vec::new();
    let mut cursor = 0usize;
    let mut paren_depth: u32 = 0;

    while cursor < tokens.len() {
        if var_block_kind(tokens[cursor].1).is_some() {
            cursor = skip_until_end_var(tokens, cursor + 1);
            continue;
        }

        let token = tokens[cursor].1;
        // Track call/grouping parenthesis depth so named arguments (`p := v`)
        // inside a call are not mistaken for statement-level assignments.
        if token.text == "(" {
            paren_depth += 1;
            cursor += 1;
            continue;
        }
        if token.text == ")" {
            paren_depth = paren_depth.saturating_sub(1);
            cursor += 1;
            continue;
        }

        if paren_depth == 0
            && token.kind == TokenKind::Identifier
            && tokens
                .get(cursor + 1)
                .is_some_and(|(_, next)| next.text == ":=")
        {
            let range = statement_range(tokens, cursor);
            statements.push(Statement {
                kind: StatementKind::Assignment,
                range,
                target: Some(token.text.clone()),
                expression: assignment_expression(tokens, cursor + 2),
            });
        } else if let Some(kind) = statement_keyword_kind(token) {
            statements.push(Statement {
                kind,
                range: statement_range(tokens, cursor),
                target: None,
                expression: None,
            });
        }

        cursor += 1;
    }

    statements
}

fn skip_until_end_var(tokens: &[(usize, &Token)], mut cursor: usize) -> usize {
    while cursor < tokens.len() {
        if tokens[cursor].1.keyword_eq("END_VAR") {
            return cursor + 1;
        }
        cursor += 1;
    }
    cursor
}

fn assignment_expression(tokens: &[(usize, &Token)], mut cursor: usize) -> Option<String> {
    let start = cursor;
    while cursor < tokens.len() && tokens[cursor].1.text != ";" {
        cursor += 1;
    }
    if start == cursor {
        None
    } else {
        Some(join_token_text(&tokens[start..cursor]))
    }
}

fn statement_range(tokens: &[(usize, &Token)], cursor: usize) -> TextRange {
    let start = tokens[cursor].1.range.start;
    let mut end = tokens[cursor].1.range.end;
    let mut search = cursor;
    while search < tokens.len() {
        end = tokens[search].1.range.end;
        if tokens[search].1.text == ";" {
            break;
        }
        search += 1;
    }
    TextRange::new(start, end)
}

fn join_token_text(tokens: &[(usize, &Token)]) -> String {
    tokens
        .iter()
        .map(|(_, token)| token.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

fn var_block_kind(token: &Token) -> Option<VarBlockKind> {
    if token.keyword_eq("VAR") {
        Some(VarBlockKind::Var)
    } else if token.keyword_eq("VAR_INPUT") {
        Some(VarBlockKind::Input)
    } else if token.keyword_eq("VAR_OUTPUT") {
        Some(VarBlockKind::Output)
    } else if token.keyword_eq("VAR_IN_OUT") {
        Some(VarBlockKind::InOut)
    } else if token.keyword_eq("VAR_GLOBAL") {
        Some(VarBlockKind::Global)
    } else if token.keyword_eq("VAR_TEMP") {
        Some(VarBlockKind::Temp)
    } else {
        None
    }
}

fn statement_keyword_kind(token: &Token) -> Option<StatementKind> {
    if token.keyword_eq("IF") {
        Some(StatementKind::If)
    } else if token.keyword_eq("CASE") {
        Some(StatementKind::Case)
    } else if token.keyword_eq("FOR") {
        Some(StatementKind::For)
    } else if token.keyword_eq("WHILE") {
        Some(StatementKind::While)
    } else if token.keyword_eq("REPEAT") {
        Some(StatementKind::Repeat)
    } else if token.keyword_eq("RETURN") {
        Some(StatementKind::Return)
    } else if token.keyword_eq("EXIT") {
        Some(StatementKind::Exit)
    } else if token.keyword_eq("CONTINUE") {
        Some(StatementKind::Continue)
    } else {
        None
    }
}

fn pou_start_kind(token: &Token) -> Option<PouKind> {
    if token.keyword_eq("PROGRAM") {
        Some(PouKind::Program)
    } else if token.keyword_eq("FUNCTION") {
        Some(PouKind::Function)
    } else if token.keyword_eq("FUNCTION_BLOCK") {
        Some(PouKind::FunctionBlock)
    } else if token.keyword_eq("ACTION") {
        Some(PouKind::Action)
    } else {
        None
    }
}

fn missing_end_diagnostic(kind: PouKind, range: TextRange) -> SyntaxDiagnostic {
    let message = match kind {
        PouKind::Program => "PROGRAM declaration is missing END_PROGRAM terminator".to_owned(),
        _ => format!(
            "{} declaration is missing {} terminator",
            kind.start_keyword(),
            kind.end_keyword()
        ),
    };

    SyntaxDiagnostic {
        code: if kind == PouKind::Program {
            "PLC0002"
        } else {
            "SYN0002"
        },
        range,
        message,
    }
}
