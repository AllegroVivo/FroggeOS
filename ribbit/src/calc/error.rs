// Mistakes, and where in the line they were found.

use core::fmt;
use super::MAX_STACK;
use super::run::MAX_LOOP_ROUNDS;
use super::value::Value;

pub enum Error<'a> {
    BadChar(char),                  // a character that isn't part of any token
    Syntax(&'static str),           // the tokens don't form an expression
    DivideByZero,
    Overflow,                       // a number or result is too big for an i64
    UnknownName(&'a str),           // a name with no value stored under it
    LoopLimit,                      // a loop repeated `MAX_LOOP_ROUNDS` times
    Cancelled,                      // program terminated by the host
    // A value of the wrong type.
    WrongType { place: &'static str, wanted: &'static str, found: &'static str},
    Undeclared(&'a str),            // assigning to a name that was not declared
    Constant(&'a str),              // assigning to a const
    // Attempting to store the wrong type into a variable.
    VariableType { name: &'a str, wanted: &'static str, found: &'static str },
    AlreadyDeclared(&'a str),       // a name is previously declared within a context
    UnknownFunction(&'a str),       // calling a name that isn't a function
    UnknownBuiltIn(&'a str, &'a str), // Calling a `Namespace.Name` that isn't defined as a builtin
    ArgumentCount { name: &'a str, wanted: usize, found: usize },
    BadArgument { name: &'a str, needs: &'static str }, // An argument a built-in cannot use
    NoReturn(&'a str),              // a function indicating a return without the `return` statement
    NoValue(&'a str),               // attempting to use the result of a void function
    BadIndex { index: i64, length: usize }, // `x[i]` used outside a valid context
    NoMember { ty: &'static str, name: &'a str }, // `.Name` is undefined on the type
    TooDeep,                        // line used more than `MAX_STACK` stack frames
    AssertFailed(Option<Value>)     // an assert's provided condition evaluates to `false`
}

// Display is used by ``println!("{}", error)``
impl fmt::Display for Error<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::BadChar(c) => write!(f, "unexpected character '{}'", c),
            Error::Syntax(message) => write!(f, "{}", message),
            Error::DivideByZero => write!(f, "division by zero"),
            Error::Overflow => write!(f, "number too big"),
            Error::UnknownName(name) => write!(f, "'{}' is not a command or variable", name),
            Error::LoopLimit => write!(f, "loop stopped after {} rounds", MAX_LOOP_ROUNDS),
            Error::Cancelled => write!(f, "keyboard interrupt (Ctrl+C) pressed"),
            Error::WrongType { place, wanted, found } => {
                write!(f, "{} needs {}, not {}", place, wanted, found)
            },
            Error::Undeclared(name) => write!(f, "'{}' isn't declared", name),
            Error::Constant(name) => write!(f, "'{}' is a constant", name),
            Error::AlreadyDeclared(name) => write!(f, "'{}' is already declared", name),
            Error::VariableType { name, wanted, found } => {
                write!(f, "'{}' needs {}, not {}", name, wanted, found)
            },
            Error::UnknownFunction(name) => write!(f, "'{}' is not a function", name),
            Error::UnknownBuiltIn(namespace, name) => {
                write!(f, "'{}.{}' is not a built-in function", namespace, name)
            },
            Error::ArgumentCount { name, wanted, found } => {
                let s = if *wanted == 1 { "" } else { "s" };
                write!(f, "'{}' takes {} argument{}, not {}", name, wanted, s, found)
            },
            Error::BadArgument { name, needs } => write!(f, "'{}' needs {}", name, needs),
            Error::NoReturn(name) => write!(f, "'{}' must return a value", name),
            Error::NoValue(name) => write!(f, "'{}' is void and has no value to be used", name),
            Error::BadIndex { index, length } => {
                write!(f, "index {} is outside a value of length {}", index, length)
            },
            Error::NoMember { ty, name } => write!(f, "{} has no member '{}'", ty, name),
            Error::TooDeep => write!(f, "nested too deeply (over {} KB of stack)", MAX_STACK / 1024),
            Error::AssertFailed(None) => write!(f, "assertion failed"),
            Error::AssertFailed(Some(message)) => write!(f, "assertion failed: {}", message),
        }
    }
}

pub struct Failure<'a> {
    pub error: Error<'a>,
    pub at: Option<usize>,
}
