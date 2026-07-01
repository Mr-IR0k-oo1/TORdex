use crate::ast::{BinExpr, Order, Program, Stmt, Value};
use crate::error::Error;
use crate::token::{BinOp, Token};

pub struct Parser {
    tokens: Vec<(Token, usize, usize)>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<(Token, usize, usize)>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse(&mut self) -> Result<Program, Error> {
        let mut statements = Vec::new();
        loop {
            self.skip_newlines();
            if self.check(Token::EOF) {
                break;
            }
            let stmt = self.parse_statement()?;
            statements.push(stmt);
        }
        Ok(Program { statements })
    }

    fn skip_newlines(&mut self) {
        while self.check(Token::Newline) {
            self.advance();
        }
    }

    fn peek(&self) -> &Token {
        &self.tokens.get(self.pos).map(|(t, _, _)| t).unwrap_or(&Token::EOF)
    }

    fn peek_linecol(&self) -> (usize, usize) {
        self.tokens.get(self.pos).map(|(_, l, c)| (*l, *c)).unwrap_or((0, 0))
    }

    fn advance(&mut self) -> &(Token, usize, usize) {
        let tok = &self.tokens[self.pos];
        self.pos += 1;
        tok
    }

    fn check(&self, expected: Token) -> bool {
        std::mem::discriminant(self.peek()) == std::mem::discriminant(&expected)
    }

    fn expect(&mut self, expected: Token) -> Result<&(Token, usize, usize), Error> {
        let (line, col) = self.peek_linecol();
        if std::mem::discriminant(self.peek()) == std::mem::discriminant(&expected) {
            Ok(self.advance())
        } else {
            Err(Error::Parse {
                line,
                col,
                message: format!("expected {:?}, got {:?}", expected, self.peek()),
            })
        }
    }

    fn parse_statement(&mut self) -> Result<Stmt, Error> {
        let tok = self.peek().clone();
        let (line, col) = self.peek_linecol();
        match &tok {
            Token::From => self.parse_from(),
            Token::Match => self.parse_match(),
            Token::Where => self.parse_where(),
            Token::Traverse => self.parse_traverse(),
            Token::Summarize => { self.advance(); Ok(Stmt::Summarize) }
            Token::Infer => self.parse_infer(),
            Token::Correlate => self.parse_correlate(),
            Token::Similar => self.parse_similar(),
            Token::Classify => self.parse_classify(),
            Token::Timeline => self.parse_timeline(),
            Token::Sort => self.parse_sort(),
            Token::Limit => self.parse_limit(),
            Token::Store => self.parse_store(),
            Token::Export => self.parse_export(),
            Token::Load => self.parse_load(),
            Token::Filter => self.parse_filter(),
            Token::Ident(_) => {
                // Could be just an expression statement, treat as FROM for shorthand
                let value = self.parse_value()?;
                // If followed by newline, it's a standalone ident expression
                Ok(Stmt::Load(value))
            }
            _ => Err(Error::Parse {
                line,
                col,
                message: format!("unexpected token {:?}, expected statement keyword", tok),
            }),
        }
    }

    fn parse_from(&mut self) -> Result<Stmt, Error> {
        self.advance(); // FROM
        let value = self.parse_value()?;
        Ok(Stmt::From(value))
    }

    fn parse_match(&mut self) -> Result<Stmt, Error> {
        self.advance(); // MATCH
        let expr = self.parse_bin_expr()?;
        Ok(Stmt::Match(expr))
    }

    fn parse_where(&mut self) -> Result<Stmt, Error> {
        self.advance(); // WHERE
        let expr = self.parse_bin_expr()?;
        Ok(Stmt::Where(expr))
    }

    fn parse_traverse(&mut self) -> Result<Stmt, Error> {
        self.advance(); // TRAVERSE
        let relation = self.parse_value()?;
        let depth = if self.check(Token::Depth) {
            self.advance();
            let depth_value = self.parse_value()?;
            match depth_value {
                Value::Integer(n) => Some(n as usize),
                _ => Some(1),
            }
        } else {
            None
        };
        Ok(Stmt::Traverse { relation, depth })
    }

    fn parse_infer(&mut self) -> Result<Stmt, Error> {
        self.advance(); // INFER
        let target = self.parse_value()?;
        Ok(Stmt::Infer(target))
    }

    fn parse_correlate(&mut self) -> Result<Stmt, Error> {
        self.advance(); // CORRELATE
        let _with = self.expect(Token::With).ok();
        let target = self.parse_value()?;
        Ok(Stmt::Correlate { target })
    }

    fn parse_similar(&mut self) -> Result<Stmt, Error> {
        self.advance(); // SIMILAR
        let _to = self.expect(Token::To).ok();
        let target = self.parse_value()?;
        Ok(Stmt::Similar { target })
    }

    fn parse_classify(&mut self) -> Result<Stmt, Error> {
        self.advance(); // CLASSIFY
        let _as = self.expect(Token::As).ok();
        let class = self.parse_value()?;
        Ok(Stmt::Classify { class })
    }

    fn parse_timeline(&mut self) -> Result<Stmt, Error> {
        self.advance(); // TIMELINE
        let from = if self.check(Token::From) {
            self.advance();
            Some(self.parse_value()?)
        } else {
            None
        };
        let to = if self.check(Token::To) {
            self.advance();
            Some(self.parse_value()?)
        } else {
            None
        };
        Ok(Stmt::Timeline { from, to })
    }

    fn parse_sort(&mut self) -> Result<Stmt, Error> {
        self.advance(); // SORT
        let _by = self.expect(Token::By).ok();
        let field = self.parse_value()?;
        let order = if self.check(Token::Asc) {
            self.advance();
            Order::Asc
        } else if self.check(Token::Desc) {
            self.advance();
            Order::Desc
        } else {
            Order::Asc
        };
        Ok(Stmt::Sort { field, order })
    }

    fn parse_limit(&mut self) -> Result<Stmt, Error> {
        self.advance(); // LIMIT
        match self.parse_value()? {
            Value::Integer(n) => Ok(Stmt::Limit(n as usize)),
            val => {
                let (_, line, col) = self.tokens.get(self.pos - 1).unwrap_or(&(Token::EOF, 0, 0));
                Err(Error::Parse {
                    line: *line, col: *col,
                    message: format!("LIMIT requires an integer, got {:?}", val),
                })
            }
        }
    }

    fn parse_store(&mut self) -> Result<Stmt, Error> {
        self.advance(); // STORE
        let _as = self.expect(Token::As).ok();
        let name = self.parse_value()?;
        Ok(Stmt::Store(name))
    }

    fn parse_export(&mut self) -> Result<Stmt, Error> {
        self.advance(); // EXPORT
        let format = self.parse_value()?;
        Ok(Stmt::Export(format))
    }

    fn parse_load(&mut self) -> Result<Stmt, Error> {
        self.advance(); // LOAD
        let source = self.parse_value()?;
        Ok(Stmt::Load(source))
    }

    fn parse_filter(&mut self) -> Result<Stmt, Error> {
        self.advance(); // FILTER
        let expr = self.parse_bin_expr()?;
        Ok(Stmt::Filter(expr))
    }

    fn parse_bin_expr(&mut self) -> Result<BinExpr, Error> {
        let lhs = self.parse_value()?;
        if self.is_bin_op() {
            let op = self.parse_bin_op()?;
            let rhs = self.parse_value()?;
            let field = lhs.as_string();
            Ok(BinExpr::Field { field, op, value: rhs })
        } else {
            Ok(BinExpr::Value(lhs))
        }
    }

    fn is_bin_op(&self) -> bool {
        matches!(
            self.peek(),
            Token::Eq | Token::Neq | Token::Ge | Token::Le | Token::Gt | Token::Lt
                | Token::And | Token::Or
        )
    }

    fn parse_bin_op(&mut self) -> Result<BinOp, Error> {
        let (line, col) = self.peek_linecol();
        match self.advance() {
            (Token::Eq, _, _) => Ok(BinOp::Eq),
            (Token::Neq, _, _) => Ok(BinOp::Neq),
            (Token::Ge, _, _) => Ok(BinOp::Ge),
            (Token::Le, _, _) => Ok(BinOp::Le),
            (Token::Gt, _, _) => Ok(BinOp::Gt),
            (Token::Lt, _, _) => Ok(BinOp::Lt),
            (Token::And, _, _) => Ok(BinOp::And),
            (Token::Or, _, _) => Ok(BinOp::Or),
            _ => Err(Error::Parse {
                line, col,
                message: format!("expected binary operator, got {:?}", self.peek()),
            }),
        }
    }

    fn parse_value(&mut self) -> Result<Value, Error> {
        let tok = self.advance().clone();
        match tok {
            (Token::Integer(v), _, _) => Ok(Value::Integer(v)),
            (Token::Float(v), _, _) => Ok(Value::Float(v)),
            (Token::Str(s), _, _) => Ok(Value::String(s)),
            (Token::Bool(b), _, _) => Ok(Value::Bool(b)),
            (Token::Null, _, _) => Ok(Value::Null),
            (Token::Ident(s), _, _) => Ok(Value::Ident(s)),
            _ => Err(Error::Parse {
                line: tok.1, col: tok.2,
                message: format!("expected value, got {:?}", tok.0),
            }),
        }
    }
}
