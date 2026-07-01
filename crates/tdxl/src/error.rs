use std::fmt;

#[derive(Debug, Clone)]
pub enum Error {
    Lex { line: usize, col: usize, message: String },
    Parse { line: usize, col: usize, message: String },
    Compile { message: String },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Lex { line, col, message } => {
                write!(f, "lex error at {}:{}: {}", line, col, message)
            }
            Error::Parse { line, col, message } => {
                write!(f, "parse error at {}:{}: {}", line, col, message)
            }
            Error::Compile { message } => write!(f, "compile error: {}", message),
        }
    }
}

impl std::error::Error for Error {}
