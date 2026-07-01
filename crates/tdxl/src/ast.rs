use crate::token::BinOp;

#[derive(Debug, Clone)]
pub enum Value {
    Integer(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Null,
    Ident(String),
}

impl Value {
    pub fn as_string(&self) -> String {
        match self {
            Value::Integer(v) => v.to_string(),
            Value::Float(v) => v.to_string(),
            Value::String(s) => s.clone(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".into(),
            Value::Ident(s) => s.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum BinExpr {
    Field { field: String, op: BinOp, value: Value },
    Expr { left: Box<BinExpr>, op: BinOp, right: Box<BinExpr> },
    Value(Value),
}

#[derive(Debug, Clone)]
pub enum Order {
    Asc,
    Desc,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    From(Value),
    Match(BinExpr),
    Where(BinExpr),
    Traverse {
        relation: Value,
        depth: Option<usize>,
    },
    Summarize,
    Infer(Value),
    Correlate {
        target: Value,
    },
    Similar {
        target: Value,
    },
    Classify {
        class: Value,
    },
    Timeline {
        from: Option<Value>,
        to: Option<Value>,
    },
    Sort {
        field: Value,
        order: Order,
    },
    Limit(usize),
    Store(Value),
    Export(Value),
    Load(Value),
    Filter(BinExpr),
}

#[derive(Debug, Clone)]
pub struct Program {
    pub statements: Vec<Stmt>,
}
