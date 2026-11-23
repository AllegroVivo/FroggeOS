// The values Ribbit works with, and the variables that hold them.

use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::string::String;
use core::fmt;
use super::error::Error;

// A value that the language works with.
#[derive(Clone)]
pub enum Value {
    Int64(i64),
    Boolean(bool),
    Char(u8),
    String(Rc<String>)
}

impl Value {
    pub(super) fn type_of(&self) -> Type {
        match self {
            Value::Int64(_) => Type::Int64,
            Value::Boolean(_) => Type::Boolean,
            Value::Char(_) => Type::Char,
            Value::String(_) => Type::String,
        }
    }

    pub fn type_name(&self) -> &'static str {
        self.type_of().name()
    }

    pub(super) fn int64<'a>(self, place: &'static str) -> Result<i64, Error<'a>> {
        match self {
            Value::Int64(n) => Ok(n),
            _ => Err(Error::WrongType { place, wanted: "Int64", found: self.type_name() }),
        }
    }

    pub(super) fn boolean<'a>(self, place: &'static str) -> Result<bool, Error<'a>> {
        match self {
            Value::Boolean(b) => Ok(b),
            _ => Err(Error::WrongType { place, wanted: "Boolean", found: self.type_name() }),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int64(n) => write!(f, "{}", n),
            Value::Boolean(true) => write!(f, "True"),
            Value::Boolean(false) => write!(f, "False"),
            Value::Char(c) => write!(f, "{}", *c as char),
            Value::String(s) => write!(f, "{}", s),
        }
    }
}

// Available types to declare a variable with.
#[derive(Clone, Copy, PartialEq)]
pub enum Type {
    Int64,
    Boolean,
    Char,
    String,
}

impl Type {
    pub(super) fn name(self) -> &'static str {
        match self {
            Type::Int64 => "Int64",
            Type::Boolean => "Boolean",
            Type::Char => "Char",
            Type::String => "String",
        }
    }

    // The value of a variable declared with no assignment.
    pub(super) fn zero(self) -> Option<Value> {
        match self {
            Type::Int64 => Some(Value::Int64(0)),
            Type::Boolean => Some(Value::Boolean(false)),
            Type::Char => Some(Value::Char(0)),
            Type::String => None,
        }
    }
}

// A single declared variable.
pub struct Variable {
    pub value: Value,
    pub constant: bool,
}

// All declared variables by name.
pub type Variables = BTreeMap<String, Variable>;
