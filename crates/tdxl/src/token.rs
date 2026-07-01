#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    From,
    Match,
    Where,
    Traverse,
    Depth,
    Summarize,
    Infer,
    Correlate,
    With,
    Similar,
    To,
    Classify,
    As,
    Timeline,
    Sort,
    By,
    Asc,
    Desc,
    Limit,
    Store,
    Export,
    Load,
    Filter,
    And,
    Or,
    Not,

    // Literals
    Integer(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Null,

    // Identifiers
    Ident(String),

    // Operators
    Eq,
    Neq,
    Ge,
    Le,
    Gt,
    Lt,
    Assign,

    // Delimiters
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Dot,
    Colon,

    Newline,
    EOF,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Eq,
    Neq,
    Ge,
    Le,
    Gt,
    Lt,
    And,
    Or,
}
