use std::fmt;
use std::fmt::Formatter;

pub type EvalError = String;

#[derive(Debug, Clone, PartialEq)]
pub enum Object {
    Integer(i64),
    Boolean(bool),
    Null,
}

impl fmt::Display for Object {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Object::Integer(i) => write!(f, "{}", i),
            Object::Null => write!(f, "null"),
            Object::Boolean(b) => write!(f, "{}", b),
        }
    }
}

