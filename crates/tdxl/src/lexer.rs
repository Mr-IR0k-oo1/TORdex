use crate::error::Error;
use crate::token::Token;

#[derive(Debug, Clone)]
pub struct Lexer {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<(Token, usize, usize)>, Error> {
        let mut tokens = Vec::new();
        loop {
            let (tok, line, col) = self.next_token()?;
            let _span = (line, col);
            match tok {
                Token::EOF => {
                    tokens.push((Token::EOF, line, col));
                    break;
                }
                _ => {
                    tokens.push((tok, line, col));
                }
            }
        }
        Ok(tokens)
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied()?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    fn peek_nth(&self, n: usize) -> Option<char> {
        self.chars.get(self.pos + n).copied()
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c == ' ' || c == '\t' || c == '\r' {
                self.advance();
            } else if c == '#' {
                while let Some(c) = self.peek() {
                    if c == '\n' {
                        break;
                    }
                    self.advance();
                }
            } else {
                break;
            }
        }
    }

    fn next_token(&mut self) -> Result<(Token, usize, usize), Error> {
        self.skip_whitespace();
        let line = self.line;
        let col = self.col;

        let c = match self.peek() {
            Some(c) => c,
            None => return Ok((Token::EOF, line, col)),
        };

        if c == '\n' {
            self.advance();
            return Ok((Token::Newline, line, col));
        }

        if c.is_ascii_digit() {
            return self.read_number(line, col);
        }

        if c == '"' {
            return self.read_string(line, col);
        }

        if c == '\'' {
            return self.read_raw_string(line, col);
        }

        if c == '_' || c.is_ascii_alphabetic() {
            return self.read_ident_or_keyword(line, col);
        }

        match c {
            '(' => { self.advance(); Ok((Token::LParen, line, col)) }
            ')' => { self.advance(); Ok((Token::RParen, line, col)) }
            '[' => { self.advance(); Ok((Token::LBracket, line, col)) }
            ']' => { self.advance(); Ok((Token::RBracket, line, col)) }
            '{' => { self.advance(); Ok((Token::LBrace, line, col)) }
            '}' => { self.advance(); Ok((Token::RBrace, line, col)) }
            ',' => { self.advance(); Ok((Token::Comma, line, col)) }
            '.' => { self.advance(); Ok((Token::Dot, line, col)) }
            ':' => { self.advance(); Ok((Token::Colon, line, col)) }

            '=' => {
                if self.peek_nth(1) == Some('=') {
                    self.advance(); self.advance();
                    Ok((Token::Eq, line, col))
                } else {
                    self.advance();
                    Ok((Token::Assign, line, col))
                }
            }

            '!' => {
                if self.peek_nth(1) == Some('=') {
                    self.advance(); self.advance();
                    Ok((Token::Neq, line, col))
                } else {
                    Err(Error::Lex { line, col, message: format!("unexpected '!'") })
                }
            }

            '>' => {
                if self.peek_nth(1) == Some('=') {
                    self.advance(); self.advance();
                    Ok((Token::Ge, line, col))
                } else {
                    self.advance();
                    Ok((Token::Gt, line, col))
                }
            }

            '<' => {
                if self.peek_nth(1) == Some('=') {
                    self.advance(); self.advance();
                    Ok((Token::Le, line, col))
                } else {
                    self.advance();
                    Ok((Token::Lt, line, col))
                }
            }

            _ => Err(Error::Lex {
                line,
                col,
                message: format!("unexpected character: '{}'", c),
            }),
        }
    }

    fn read_number(&mut self, line: usize, col: usize) -> Result<(Token, usize, usize), Error> {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || c == '.' {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }
        if s.contains('.') {
            let v: f64 = s.parse().unwrap_or(0.0);
            Ok((Token::Float(v), line, col))
        } else {
            let v: i64 = s.parse().unwrap_or(0);
            Ok((Token::Integer(v), line, col))
        }
    }

    fn read_string(&mut self, line: usize, col: usize) -> Result<(Token, usize, usize), Error> {
        self.advance(); // opening "
        let mut s = String::new();
        loop {
            match self.advance() {
                Some('"') => break,
                Some('\\') => match self.advance() {
                    Some('n') => s.push('\n'),
                    Some('t') => s.push('\t'),
                    Some('r') => s.push('\r'),
                    Some('\\') => s.push('\\'),
                    Some('"') => s.push('"'),
                    Some(c) => s.push(c),
                    None => return Err(Error::Lex {
                        line: self.line, col: self.col,
                        message: "unterminated string escape".into(),
                    }),
                },
                Some(c) => s.push(c),
                None => return Err(Error::Lex {
                    line: self.line, col: self.col,
                    message: "unterminated string".into(),
                }),
            }
        }
        Ok((Token::Str(s), line, col))
    }

    fn read_raw_string(&mut self, line: usize, col: usize) -> Result<(Token, usize, usize), Error> {
        self.advance(); // opening '
        let mut s = String::new();
        loop {
            match self.advance() {
                Some('\'') => break,
                Some(c) => s.push(c),
                None => return Err(Error::Lex {
                    line: self.line, col: self.col,
                    message: "unterminated raw string".into(),
                }),
            }
        }
        Ok((Token::Str(s), line, col))
    }

    fn read_ident_or_keyword(&mut self, line: usize, col: usize) -> Result<(Token, usize, usize), Error> {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }
        let token = match s.as_str() {
            "FROM" | "from" | "From" => Token::From,
            "MATCH" | "match" | "Match" => Token::Match,
            "WHERE" | "where" | "Where" => Token::Where,
            "TRAVERSE" | "traverse" | "Traverse" => Token::Traverse,
            "DEPTH" | "depth" | "Depth" => Token::Depth,
            "SUMMARIZE" | "summarize" | "Summarize" => Token::Summarize,
            "INFER" | "infer" | "Infer" => Token::Infer,
            "CORRELATE" | "correlate" | "Correlate" => Token::Correlate,
            "WITH" | "with" | "With" => Token::With,
            "SIMILAR" | "similar" | "Similar" => Token::Similar,
            "TO" | "to" | "To" => Token::To,
            "CLASSIFY" | "classify" | "Classify" => Token::Classify,
            "AS" | "as" | "As" => Token::As,
            "TIMELINE" | "timeline" | "Timeline" => Token::Timeline,
            "SORT" | "sort" | "Sort" => Token::Sort,
            "BY" | "by" | "By" => Token::By,
            "ASC" | "asc" | "Asc" => Token::Asc,
            "DESC" | "desc" | "Desc" => Token::Desc,
            "LIMIT" | "limit" | "Limit" => Token::Limit,
            "STORE" | "store" | "Store" => Token::Store,
            "EXPORT" | "export" | "Export" => Token::Export,
            "LOAD" | "load" | "Load" => Token::Load,
            "FILTER" | "filter" | "Filter" => Token::Filter,
            "AND" | "and" | "And" => Token::And,
            "OR" | "or" | "Or" => Token::Or,
            "NOT" | "not" | "Not" => Token::Not,
            "true" | "True" | "TRUE" => Token::Bool(true),
            "false" | "False" | "FALSE" => Token::Bool(false),
            "null" | "Null" | "NULL" => Token::Null,
            _ => Token::Ident(s),
        };
        Ok((token, line, col))
    }
}
