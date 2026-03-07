use crate::diagnostic::{Diagnostic, Position};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Import,
    Use,
    Fn,
    Package,
    Body,
    Type,
    Enum,
    Record,
    Const,
    Return,
    Requires,
    Ensures,
    Global,
    Depends,
    For,
    In,
    If,
    Else,
    Case,
    When,
    While,
    Assert,
    Invariant,
    Increases,
    Decreases,
    Null,
    Then,
    And,
    Or,
    Not,
    True,
    False,
    Identifier(String),
    Integer(String),
    Float(String),
    Character(char),
    String(String),
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Semicolon,
    Dot,
    DotDot,
    Arrow,
    FatArrow,
    Assign,
    EqualEqual,
    BangEqual,
    Plus,
    Minus,
    Star,
    Slash,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Eof,
}

pub fn lex(source: &str) -> Result<Vec<Token>, Diagnostic> {
    let mut lexer = Lexer::new(source);
    lexer.lex_all()
}

struct Lexer<'a> {
    source: &'a [u8],
    index: usize,
    line: usize,
    column: usize,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source: source.as_bytes(),
            index: 0,
            line: 1,
            column: 1,
        }
    }

    fn lex_all(&mut self) -> Result<Vec<Token>, Diagnostic> {
        let mut tokens = Vec::new();
        loop {
            self.skip_ignored();
            let position = self.position();
            let Some(byte) = self.peek() else {
                tokens.push(Token {
                    kind: TokenKind::Eof,
                    position,
                });
                return Ok(tokens);
            };

            let kind = match byte {
                b'(' => {
                    self.bump();
                    TokenKind::LParen
                }
                b')' => {
                    self.bump();
                    TokenKind::RParen
                }
                b'[' => {
                    self.bump();
                    TokenKind::LBracket
                }
                b']' => {
                    self.bump();
                    TokenKind::RBracket
                }
                b'{' => {
                    self.bump();
                    TokenKind::LBrace
                }
                b'}' => {
                    self.bump();
                    TokenKind::RBrace
                }
                b',' => {
                    self.bump();
                    TokenKind::Comma
                }
                b';' => {
                    self.bump();
                    TokenKind::Semicolon
                }
                b'.' => {
                    self.bump();
                    if self.peek() == Some(b'.') {
                        self.bump();
                        TokenKind::DotDot
                    } else {
                        TokenKind::Dot
                    }
                }
                b'+' => {
                    self.bump();
                    TokenKind::Plus
                }
                b'-' => {
                    self.bump();
                    if self.peek() == Some(b'>') {
                        self.bump();
                        TokenKind::Arrow
                    } else {
                        TokenKind::Minus
                    }
                }
                b'*' => {
                    self.bump();
                    TokenKind::Star
                }
                b'/' => {
                    self.bump();
                    TokenKind::Slash
                }
                b'=' => {
                    self.bump();
                    if self.peek() == Some(b'=') {
                        self.bump();
                        TokenKind::EqualEqual
                    } else if self.peek() == Some(b'>') {
                        self.bump();
                        TokenKind::FatArrow
                    } else {
                        TokenKind::Assign
                    }
                }
                b'!' => {
                    self.bump();
                    if self.peek() == Some(b'=') {
                        self.bump();
                        TokenKind::BangEqual
                    } else {
                        return Err(Diagnostic::new(
                            "unexpected `!`; use `!=` for inequality",
                            position,
                        ));
                    }
                }
                b'<' => {
                    self.bump();
                    if self.peek() == Some(b'=') {
                        self.bump();
                        TokenKind::LessEqual
                    } else {
                        TokenKind::Less
                    }
                }
                b'>' => {
                    self.bump();
                    if self.peek() == Some(b'=') {
                        self.bump();
                        TokenKind::GreaterEqual
                    } else {
                        TokenKind::Greater
                    }
                }
                b'"' => TokenKind::String(self.lex_string(position)?),
                b'\'' => TokenKind::Character(self.lex_character(position)?),
                b'0'..=b'9' => self.lex_number(),
                b'A'..=b'Z' | b'a'..=b'z' | b'_' => self.lex_identifier_or_keyword(),
                _ => {
                    return Err(Diagnostic::new(
                        format!("unexpected character `{}`", byte as char),
                        position,
                    ));
                }
            };

            tokens.push(Token { kind, position });
        }
    }

    fn skip_ignored(&mut self) {
        loop {
            while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
                self.bump();
            }

            if self.peek() == Some(b'/') && self.peek_next() == Some(b'/') {
                while let Some(byte) = self.peek() {
                    self.bump();
                    if byte == b'\n' {
                        break;
                    }
                }
                continue;
            }

            break;
        }
    }

    fn lex_string(&mut self, position: Position) -> Result<String, Diagnostic> {
        self.bump();
        let mut value = String::new();
        while let Some(byte) = self.peek() {
            match byte {
                b'"' => {
                    self.bump();
                    return Ok(value);
                }
                b'\\' => {
                    self.bump();
                    let Some(escaped) = self.peek() else {
                        return Err(Diagnostic::new("unterminated escape sequence", position));
                    };
                    let translated = match escaped {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        _ => {
                            return Err(Diagnostic::new(
                                format!("unsupported escape sequence `\\{}`", escaped as char),
                                position,
                            ));
                        }
                    };
                    self.bump();
                    value.push(translated);
                }
                b'\n' => {
                    return Err(Diagnostic::new(
                        "unterminated string literal",
                        self.position(),
                    ));
                }
                _ => {
                    self.bump();
                    value.push(byte as char);
                }
            }
        }

        Err(Diagnostic::new("unterminated string literal", position))
    }

    fn lex_character(&mut self, position: Position) -> Result<char, Diagnostic> {
        self.bump();
        let Some(byte) = self.peek() else {
            return Err(Diagnostic::new("unterminated character literal", position));
        };
        if matches!(byte, b'\'' | b'\n') {
            return Err(Diagnostic::new(
                "character literals must contain exactly one character",
                position,
            ));
        }

        self.bump();
        let value = byte as char;

        match self.peek() {
            Some(b'\'') => {
                self.bump();
                Ok(value)
            }
            Some(b'\n') | None => Err(Diagnostic::new("unterminated character literal", position)),
            _ => Err(Diagnostic::new(
                "character literals must contain exactly one character",
                position,
            )),
        }
    }

    fn lex_number(&mut self) -> TokenKind {
        let start = self.index;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.bump();
        }

        if self.peek() == Some(b'.') && matches!(self.peek_next(), Some(b'0'..=b'9')) {
            self.bump();
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.bump();
            }

            return TokenKind::Float(
                String::from_utf8(self.source[start..self.index].to_vec()).expect("float is ASCII"),
            );
        }

        TokenKind::Integer(
            String::from_utf8(self.source[start..self.index].to_vec()).expect("integer is ASCII"),
        )
    }

    fn lex_identifier_or_keyword(&mut self) -> TokenKind {
        let start = self.index;
        while matches!(
            self.peek(),
            Some(b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_')
        ) {
            self.bump();
        }
        let text = String::from_utf8(self.source[start..self.index].to_vec())
            .expect("identifier is ASCII");
        match text.as_str() {
            "import" => TokenKind::Import,
            "use" => TokenKind::Use,
            "fn" => TokenKind::Fn,
            "package" => TokenKind::Package,
            "body" => TokenKind::Body,
            "type" => TokenKind::Type,
            "enum" => TokenKind::Enum,
            "record" => TokenKind::Record,
            "const" => TokenKind::Const,
            "return" => TokenKind::Return,
            "requires" => TokenKind::Requires,
            "ensures" => TokenKind::Ensures,
            "global" => TokenKind::Global,
            "depends" => TokenKind::Depends,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "case" => TokenKind::Case,
            "when" => TokenKind::When,
            "while" => TokenKind::While,
            "assert" => TokenKind::Assert,
            "invariant" => TokenKind::Invariant,
            "increases" => TokenKind::Increases,
            "decreases" => TokenKind::Decreases,
            "null" => TokenKind::Null,
            "then" => TokenKind::Then,
            "and" => TokenKind::And,
            "or" => TokenKind::Or,
            "not" => TokenKind::Not,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            _ => TokenKind::Identifier(text),
        }
    }

    fn peek(&self) -> Option<u8> {
        self.source.get(self.index).copied()
    }

    fn peek_next(&self) -> Option<u8> {
        self.source.get(self.index + 1).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.index += 1;
        if byte == b'\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(byte)
    }

    fn position(&self) -> Position {
        Position {
            line: self.line,
            column: self.column,
            offset: self.index,
        }
    }
}
