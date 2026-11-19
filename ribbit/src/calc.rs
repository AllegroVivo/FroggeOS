// A calculator for integer expressions.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

// A value that the language works with.
#[derive(Clone, Copy)]
pub enum Value {
    Int64(i64),
    Boolean(bool),
}

impl Value {
    fn type_of(self) -> Type {
        match self {
            Value::Int64(_) => Type::Int64,
            Value::Boolean(_) => Type::Boolean,
        }
    }

    pub fn type_name(self) -> &'static str {
        self.type_of().name()
    }

    fn int64<'a>(self, place: &'static str) -> Result<i64, Error<'a>> {
        match self {
            Value::Int64(n) => Ok(n),
            _ => Err(Error::WrongType { place, wanted: "Int64", found: self.type_name() }),
        }
    }

    fn boolean<'a>(self, place: &'static str) -> Result<bool, Error<'a>> {
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
        }
    }
}

// Available types to declare a variable with.
#[derive(Clone, Copy, PartialEq)]
pub enum Type {
    Int64,
    Boolean,
}

impl Type {
    fn name(self) -> &'static str {
        match self {
            Type::Int64 => "Int64",
            Type::Boolean => "Boolean",
        }
    }

    // The value of a variable declared with no assignment.
    fn zero(self) -> Value {
        match self {
            Type::Int64 => Value::Int64(0),
            Type::Boolean => Value::Boolean(false),
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

// A declared function.
pub struct Function<'a> {
    returns: Option<Type>,          // None for void
    params: Vec<(&'a str, Type)>,
    body: Vec<Stmt<'a>>,
}

// All declared functions by name.
pub type Functions = BTreeMap<String, Function<'static>>;

// Variables seen by the currently executing code.
struct Env<'a, 'v> {
    globals: &'v mut Variables,
    functions: &'v Functions,
    locals: Vec<(&'a str, Variable)>,
    depth: usize,
    stack_start: usize                  // where the stack was when the line began
}

// The most stack a line may use while it runs.
const MAX_STACK: usize = 40 * 1024;

// Roughly where the stack is now.
fn stack_address() -> usize {
    let marker = 0u8;
    &marker as *const u8 as usize
}

impl<'a> Env<'a, '_> {
    fn check_stack(&self) -> Result<(), Error<'a>> {
        if self.stack_start.saturating_sub(stack_address()) > MAX_STACK {
            return Err(Error::TooDeep);
        }
        Ok(())
    }

    fn get(&self, name: &str) -> Option<&Variable> {
        match self.locals.iter().rev().find(|(local, _)| *local == name) {
            Some((_, variable)) => Some(variable),
            None => self.globals.get(name),
        }
    }

    fn get_mut(&mut self, name: &str) -> Option<&mut Variable> {
        match self.locals.iter_mut().rev().find(|(local, _)| *local == name) {
            Some((_, variable)) => Some(variable),
            None => self.globals.get_mut(name),
        }
    }

    fn declare(&mut self, name: &'a str, variable: Variable) -> Result<(), Error<'a>> {
        if self.depth == 0 {
            // Declared at the prompt. Replaces any existing variable.
            self.globals.insert(String::from(name), variable);
        } else if self.locals.iter().any(|(local, _)| *local == name) {
            // Redeclared inside a block. Not allowed.
            return Err(Error::AlreadyDeclared(name));
        } else {
            self.locals.push((name, variable));
        }
        Ok(())
    }
}

pub enum Error<'a> {
    BadChar(char),              // a character that isn't part of any token
    Syntax(&'static str),       // the tokens don't form an expression
    DivideByZero,
    Overflow,                   // a number or result is too big for an i64
    UnknownName(&'a str),       // a name with no value stored under it
    LoopLimit,                  // a loop repeated ``MAX_LOOP_ROUNDS`` times
    // A value of the wrong type.
    WrongType { place: &'static str, wanted: &'static str, found: &'static str},
    Undeclared(&'a str),        // assigning to a name that was not declared
    Constant(&'a str),          // assigning to a const
    // Attempting to store the wrong type into a variable.
    VariableType { name: &'a str, wanted: &'static str, found: &'static str },
    AlreadyDeclared(&'a str),   // a name is previously declared within a context
    UnknownFunction(&'a str),   // calling a name that isn't a function
    ArgumentCount { name: &'a str, wanted: usize, found: usize },
    NoReturn(&'a str),          // a function indicating a return without the `return` statement
    NoValue(&'a str),           // attempting to use the result of a void function
    TooDeep,                    // line used more than `MAX_STACK` stack frames
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
            Error::ArgumentCount { name, wanted, found } => {
                let s = if *wanted == 1 { "" } else { "s" };
                write!(f, "'{}' takes {} argument{}, not {}", name, wanted, s, found)
            },
            Error::NoReturn(name) => write!(f, "'{}' must return a value", name),
            Error::NoValue(name) => write!(f, "'{}' is void and has no value to be used", name),
            Error::TooDeep => write!(f, "nested too deeply (over {} KB of stack)", MAX_STACK / 1024),
        }
    }
}

// ``derive`` instructs the compiler to write these traits automatically.
// ``Copy`` allows a token to be copied in the same manner as an integer.
// ``PartialEq`` allows for comparison with ``==``.
#[derive(Clone, Copy, PartialEq)]
enum Token<'a> {
    Number(i64),        // 420
    Name(&'a str),      // Name
    Plus,               // +
    Minus,              // -
    Star,               // *
    Slash,              // /
    Percent,            // %
    LeftParen,          // (
    RightParen,         // )
    Equals,             // =
    EqualEqual,         // ==
    NotEqual,           // !=
    Less,               // <
    LessEqual,          // <=
    Greater,            // >
    GreaterEqual,       // >=
    Semicolon,          // ;
    LeftBrace,          // {
    RightBrace,         // }
    If,                 // if
    Else,               // else
    While,              // while
    Bang,               // !
    AndAnd,             // &&
    OrOr,               // ||
    True,               // True
    False,              // False
    Var,                // var
    Const,              // const
    Type(Type),         // I64
    Comma,              // ,
    Return,             // return
    Void,               // Void
    For,                // for
    Break,              // break
    Continue,           // continue
    Compound(Op),       // +=
    PlusPlus,           // ++
    MinusMinus,         // --
    Amp,                // &
    Pipe,               // |
    Caret,              // ^
    Tilde,              // ~
    ShiftLeft,          // <<
    ShiftRight,         // >>
}

#[allow(clippy::needless_lifetimes)]
fn tokenize<'a>(text: &'a str) -> Result<Vec<Token<'a>>, Error<'a>> {
    let mut tokens = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        let token = match c {
            ' ' => continue,
            '+' if chars.next_if(|&(_, d)| d == '+').is_some() => Token::PlusPlus,
            '-' if chars.next_if(|&(_, d)| d == '-').is_some() => Token::MinusMinus,
            '+' if chars.next_if(|&(_, d)| d == '=').is_some() => Token::Compound(Op::Add),
            '-' if chars.next_if(|&(_, d)| d == '=').is_some() => Token::Compound(Op::Subtract),
            '*' if chars.next_if(|&(_, d)| d == '=').is_some() => Token::Compound(Op::Multiply),
            '/' if chars.next_if(|&(_, d)| d == '=').is_some() => Token::Compound(Op::Divide),
            '%' if chars.next_if(|&(_, d)| d == '=').is_some() => Token::Compound(Op::Remainder),
            '+' => Token::Plus,
            '-' => Token::Minus,
            '*' => Token::Star,
            '/' => Token::Slash,
            '%' => Token::Percent,
            '(' => Token::LeftParen,
            ')' => Token::RightParen,
            ';' => Token::Semicolon,
            '{' => Token::LeftBrace,
            '}' => Token::RightBrace,
            ',' => Token::Comma,
            '&' if chars.next_if(|&(_, d)| d == '&').is_some() => Token::AndAnd,
            '|' if chars.next_if(|&(_, d)| d == '|').is_some() => Token::OrOr,
            '&' if chars.next_if(|&(_, d)| d == '=').is_some() => Token::Compound(Op::BitAnd),
            '|' if chars.next_if(|&(_, d)| d == '=').is_some() => Token::Compound(Op::BitOr),
            '^' if chars.next_if(|&(_, d)| d == '=').is_some() => Token::Compound(Op::BitXor),
            '&' => Token::Amp,
            '|' => Token::Pipe,
            '^' => Token::Caret,
            '~' => Token::Tilde,
            '<' | '>' if chars.next_if(|&(_, d)| d == c).is_some() => {
                let equals_next = chars.next_if(|&(_, d)| d == '=').is_some();
                match(c, equals_next) {
                    ('<', false) => Token::ShiftLeft,
                    ('<', true) => Token::Compound(Op::ShiftLeft),
                    ('>', false) => Token::ShiftRight,
                    ('>', true) => Token::Compound(Op::ShiftRight),
                    _ => return Err(Error::BadChar(c)),
                }
            },
            '=' | '!' | '<' | '>' => {
                // NOTE: ``==``, ``!=``, ``<=`` and ``>=`` are two characters in one token.
                let equals_next = chars.next_if(|&(_, d)| d == '=').is_some();
                match (c, equals_next) {
                    ('=', false) => Token::Equals,
                    ('=', true) => Token::EqualEqual,
                    ('!', false) => Token::Bang,
                    ('!', true) => Token::NotEqual,
                    ('<', false) => Token::Less,
                    ('<', true) => Token::LessEqual,
                    ('>', false) => Token::Greater,
                    ('>', true) => Token::GreaterEqual,
                    _ => return Err(Error::BadChar(c)),
                }
            },
            '0' if chars.next_if(|&(_, d)| d == 'x' || d == 'X').is_some() => {
                let mut value: i64 = 0;
                let mut digits = 0;
                while let Some(digit) = chars.peek().and_then(|&(_, d)| d.to_digit(16)) {
                    chars.next();
                    digits += 1;
                    value = value
                        .checked_mul(16)
                        .and_then(|v| v.checked_add(digit as i64))
                        .ok_or(Error::Overflow)?;
                }
                if digits == 0 {
                    return Err(Error::Syntax("expected hex digits after '0x'"));
                }
                Token::Number(value)
            },
            '0'..='9' => {
                let mut value = (c as u8 - b'0') as i64;
                while let Some(&(_, d)) = chars.peek() {
                    if !d.is_ascii_digit() {
                        break;
                    }
                    chars.next();
                    let digit = (d as u8 - b'0') as i64;
                    value = value
                        .checked_mul(10)
                        .and_then(|v| v.checked_add(digit))
                        .ok_or(Error::Overflow)?;
                }
                Token::Number(value)
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let mut end = start + 1;
                while let Some(&(i, d)) = chars.peek() {
                    if !(d.is_ascii_alphanumeric() || d == '_') {
                        break;
                    }
                    chars.next();
                    end = i + 1;
                }
                match &text[start..end] {
                    "if" => Token::If,
                    "else" => Token::Else,
                    "while" => Token::While,
                    "true" => Token::True,
                    "false" => Token::False,
                    "var" => Token::Var,
                    "const" => Token::Const,
                    "Int64" => Token::Type(Type::Int64),
                    "Boolean" => Token::Type(Type::Boolean),
                    "return" => Token::Return,
                    "Void" => Token::Void,
                    "for" => Token::For,
                    "break" => Token::Break,
                    "continue" => Token::Continue,
                    name => Token::Name(name),
                }
            }
            other => return Err(Error::BadChar(other))
        };
        tokens.push(token);
    }
    Ok(tokens)
}

#[derive(Clone, Copy, PartialEq)]
enum Op {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    BitAnd,
    BitOr,
    BitXor,
    ShiftLeft,
    ShiftRight,
}

impl Op {
    fn symbol(&self) -> &'static str {
        match self {
            Op::Add => "'+'",
            Op::Subtract => "'-'",
            Op::Multiply => "'*'",
            Op::Divide => "'/'",
            Op::Remainder => "'%'",
            Op::Equal => "'=='",
            Op::NotEqual => "'!='",
            Op::Less => "'<'",
            Op::LessEqual => "'<='",
            Op::Greater => "'>'",
            Op::GreaterEqual => "'>='",
            Op::BitAnd => "'&'",
            Op::BitOr => "'|'",
            Op::BitXor => "'^'",
            Op::ShiftLeft => "'<<'",
            Op::ShiftRight => "'>>'",
        }
    }
}

enum Infix {
    Op(Op),
    And,
    Or,
}

fn infix(token: Token<'_>) -> Option<(u8, Infix)> {
    Some(match token {
        Token::OrOr => (1, Infix::Or),
        Token::AndAnd => (2, Infix::And),
        Token::Pipe => (3, Infix::Op(Op::BitOr)),
        Token::Caret => (4, Infix::Op(Op::BitXor)),
        Token::Amp => (5, Infix::Op(Op::BitAnd)),
        Token::EqualEqual => (6, Infix::Op(Op::Equal)),
        Token::NotEqual => (6, Infix::Op(Op::NotEqual)),
        Token::Less => (7, Infix::Op(Op::Less)),
        Token::LessEqual => (7, Infix::Op(Op::LessEqual)),
        Token::Greater => (7, Infix::Op(Op::Greater)),
        Token::GreaterEqual => (7, Infix::Op(Op::GreaterEqual)),
        Token::ShiftLeft => (8, Infix::Op(Op::ShiftLeft)),
        Token::ShiftRight => (8, Infix::Op(Op::ShiftRight)),
        Token::Plus => (9, Infix::Op(Op::Add)),
        Token::Minus => (9, Infix::Op(Op::Subtract)),
        Token::Star => (10, Infix::Op(Op::Multiply)),
        Token::Slash => (10, Infix::Op(Op::Divide)),
        Token::Percent => (10, Infix::Op(Op::Remainder)),
        _ => return None,
    })
}

enum Expr<'a> {
    Literal(Value),
    Variable(&'a str),
    Negate(Box<Expr<'a>>),
    Not(Box<Expr<'a>>),
    BitNot(Box<Expr<'a>>),
    Binary(Op, Box<Expr<'a>>, Box<Expr<'a>>),
    And(Box<Expr<'a>>, Box<Expr<'a>>),
    Or(Box<Expr<'a>>, Box<Expr<'a>>),
    Call(&'a str, Vec<Expr<'a>>),
}

enum Stmt<'a> {
    Expr(Expr<'a>),
    Assign(&'a str, Expr<'a>),
    Declare { name: &'a str, ty: Option<Type>, value: Expr<'a>, constant: bool },
    If(Expr<'a>, Vec<Stmt<'a>>, Vec<Stmt<'a>>), // condition, then, else
    Return(Option<Expr<'a>>),
    Break,
    Continue,
    Loop {
        keyword: &'static str,
        init: Vec<Stmt<'a>>,
        condition: Expr<'a>,
        step: Vec<Stmt<'a>>,
        body: Vec<Stmt<'a>>,
    },
}

fn shorthand<'a>(name: &'a str, op: Op, value: Expr<'a>) -> Stmt<'a> {
    Stmt::Assign(name, Expr::Binary(op, Box::new(Expr::Variable(name)), Box::new(value)))
}

struct Parser<'a> {
    tokens: Vec<Token<'a>>,
    pos: usize,
    depth: usize,
    in_function: bool,
    loops: usize,
    functions: Vec<(&'a str, Function<'a>)>,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<Token<'a>> {
        self.tokens.get(self.pos).copied()
    }

    fn peek_at(&self, n: usize) -> Option<Token<'a>> {
        self.tokens.get(self.pos + n).copied()
    }

    fn next(&mut self) -> Option<Token<'a>> {
        let token = self.peek();
        self.pos += 1;
        token
    }

    // Take the next token if ``expected``, else fail with ``message``.
    fn expect(&mut self, expected: Token<'a>, message: &'static str) -> Result<(), Error<'a>> {
        if self.next() != Some(expected) {
            return Err(Error::Syntax(message));
        }
        Ok(())
    }

    // A whole line, or the inside of a block. Statements are separated by `;`.
    // The `;` can be left out before a `}` or the end of the line, and after
    // an if or while, which already end with a `}`.
    fn statements(&mut self) -> Result<Vec<Stmt<'a>>, Error<'a>> {
        let mut list = Vec::new();
        loop {
            match (self.peek(), self.peek_at(1), self.peek_at(2)) {
                (None | Some(Token::RightBrace), _, _) => return Ok(list),
                (Some(Token::Semicolon), _, _) => self.pos += 1,
                (Some(Token::Type(_) | Token::Void), Some(Token::Name(_)), Some(Token::LeftParen)) => {
                    self.function()?;
                },
                _ => {
                    let stmt = self.statement()?;
                    let ends_with_block = matches!(stmt, Stmt::If(..) | Stmt::Loop { .. });
                    list.push(stmt);
                    let separator_next =
                        matches!(self.peek(), None | Some(Token::Semicolon | Token::RightBrace));
                    if !ends_with_block && !separator_next {
                        return Err(Error::Syntax("expected ';'"));
                    }
                }
            }
        }
    }

    fn statement(&mut self) -> Result<Stmt<'a>, Error<'a>> {
        // Look at the next two tokens. ``=`` starts an assignment, ``==`` does not.
        match (self.peek(), self.tokens.get(self.pos + 1).copied()) {
            (Some(Token::If), _) => {
                self.pos += 1;
                let condition = self.condition()?;
                let then_block = self.block()?;
                let else_block = if self.peek() == Some(Token::Else) {
                    self.pos += 1;
                    if self.peek() == Some(Token::If) {
                        vec![self.statement()?] // (else if)
                    } else {
                        self.block()?
                    }
                } else {
                    Vec::new()
                };
                Ok(Stmt::If(condition, then_block, else_block))
            },
            (Some(Token::While), _) => {
                self.pos += 1;
                let condition = self.condition()?;
                let body = self.loop_body()?;
                Ok(Stmt::Loop { keyword: "while", init: Vec::new(), condition, step: Vec::new(), body })
            },
            (Some(Token::For), _) => {
                self.pos += 1;
                self.expect(Token::LeftParen, "expected '(' after 'for'")?;
                let init = self.for_part(Token::Semicolon, "expected ';'")?;
                // No condition loops go until break - e.g. `for (;;)`
                let condition = match self.peek() {
                    Some(Token::Semicolon) => Expr::Literal(Value::Boolean(true)),
                    _ => self.expression()?,
                };
                self.expect(Token::Semicolon, "expected ';'")?;
                let step = self.for_part(Token::RightParen, "expected ')'")?;
                let body = self.loop_body()?;
                Ok(Stmt::Loop { keyword: "for", init, condition, step, body })
            },
            (Some(Token::Return), _) => {
                self.pos += 1;
                if !self.in_function {
                    return Err(Error::Syntax("return is only allowed within a function body"));
                }
                // A plain `return` gives back nothing.
                let value = match self.peek() {
                    None | Some(Token::Semicolon | Token::RightBrace) => None,
                    _ => Some(self.expression()?),
                };
                Ok(Stmt::Return(value))
            },
            (Some(token @ (Token::Break | Token::Continue)), _) => {
                self.pos += 1;
                if self.loops == 0 {
                    return Err(Error::Syntax("break and continue are only allowed within a loop body"));
                }
                Ok(if token == Token::Break { Stmt::Break } else { Stmt::Continue })
            },
            (Some(Token::Void), _) => Err(Error::Syntax("cannot assign void to a variable")),
            (Some(Token::Type(_) | Token::Var | Token::Const), _) => self.declaration(),
            (Some(Token::Name(name)), Some(Token::Equals)) => {
                self.pos += 2;
                Ok(Stmt::Assign(name, self.expression()?))
            },
            (Some(Token::Name(name)), Some(Token::Compound(op))) => {
                self.pos += 2;
                Ok(shorthand(name, op, self.expression()?))
            },
            (Some(Token::Name(name)), Some(token @ (Token::PlusPlus | Token::MinusMinus))) => {
                self.pos += 2;
                let op = if token == Token::PlusPlus { Op::Add } else { Op::Subtract };
                Ok(shorthand(name, op, Expr::Literal(Value::Int64(1))))
            },
            _ => Ok(Stmt::Expr(self.expression()?))
        }
    }

    fn declaration(&mut self) -> Result<Stmt<'a>, Error<'a>> {
        let constant = self.peek() == Some(Token::Const);
        if constant {
            self.pos += 1;
        }
        let ty = match self.next() {
            Some(Token::Type(ty)) => Some(ty),
            Some(Token::Var) if !constant => None,
            _ => return Err(Error::Syntax("expected a type")),
        };
        let Some(Token::Name(name)) = self.next() else {
            return Err(Error::Syntax("expected a name"));
        };
        let value = match (self.peek(), ty) {
            (Some(Token::Equals), _) => {
                self.pos += 1;
                self.expression()?
            }
            (_, Some(ty)) if !constant => Expr::Literal(ty.zero()),
            _ => return Err(Error::Syntax("expected '='")),
        };
        Ok(Stmt::Declare {name, ty, value, constant })
    }

    fn function(&mut self) -> Result<(), Error<'a>> {
        let returns = match self.next() {
            Some(Token::Type(ty)) => Some(ty),
            _ => None,
        };
        let Some(Token::Name(name)) = self.next() else {
            return Err(Error::Syntax("expected a name"))
        };
        if self.depth > 0 {
            return Err(Error::Syntax("functions may only be declared at the prompt"));
        }
        self.expect(Token::LeftParen, "expected '('")?;
        let mut params = Vec::new();
        while self.peek() != Some(Token::RightParen) {
            if !params.is_empty() {
                self.expect(Token::Comma, "expected ',' or ')'")?;
            }
            let Some(Token::Type(ty)) = self.next() else {
                return Err(Error::Syntax("expected a parameter type"));
            };
            let Some(Token::Name(param)) = self.next() else {
                return Err(Error::Syntax("expected a parameter name"));
            };
            if params.iter().any(|(other, _)| *other == param) {
                return Err(Error::AlreadyDeclared(param));
            }
            params.push((param, ty));
        }
        self.pos += 1; // the ')'
        self.in_function = true;
        let body = self.block()?;
        self.in_function = false;
        self.functions.push((name, Function { returns, params, body }));
        Ok(())
    }

    // The condition of an ``if`` or ``while``.
    fn condition(&mut self) -> Result<Expr<'a>, Error<'a>> {
        self.expect(Token::LeftParen, "expected '(' after if or while")?;
        let condition = self.expression()?;
        self.expect(Token::RightParen, "expected ')'")?;
        Ok(condition)
    }

    // The body of a `while` or `for` loop.
    fn loop_body(&mut self) -> Result<Vec<Stmt<'a>>, Error<'a>> {
        self.loops += 1;
        let body = self.block()?;
        self.loops -= 1;
        Ok(body)
    }

    fn for_part(&mut self, end: Token<'a>, message: &'static str) -> Result<Vec<Stmt<'a>>, Error<'a>> {
        let part = if self.peek() == Some(end) { Vec::new() } else { vec![self.statement()?] };
        self.expect(end, message)?;
        Ok(part)
    }
    
    fn block(&mut self) -> Result<Vec<Stmt<'a>>, Error<'a>> {
        self.expect(Token::LeftBrace, "expected '{'")?;
        self.depth += 1;
        let body = self.statements()?;
        self.depth -= 1;
        self.expect(Token::RightBrace, "expected '}'")?;
        Ok(body)
    }

    fn expression(&mut self) -> Result<Expr<'a>, Error<'a>> {
        self.binary(1)
    }

    // Reads operands joined by binary operators, but only operators
    // that bind at least as tightly as `min`.
    fn binary(&mut self, min: u8) -> Result<Expr<'a>, Error<'a>> {
        let mut left = self.unary()?;
        while let Some((precedence, infix)) = self.peek().and_then(infix) {
            if precedence < min {
                break;
            }
            self.pos += 1;
            // Right side only takes operators that bind tighter.
            let right = self.binary(precedence + 1)?;
            let (left_side, right) = (Box::new(left), Box::new(right));
            left = match infix {
                Infix::Op(op) => Expr::Binary(op, left_side, right),
                Infix::And => Expr::And(left_side, right),
                Infix::Or => Expr::Or(left_side, right),
            };
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Expr<'a>, Error<'a>> {
        if self.peek() == Some(Token::Minus) {
            self.pos += 1;
            return Ok(Expr::Negate(Box::new(self.unary()?)));
        }
        if self.peek() == Some(Token::Bang) {
            self.pos += 1;
            return Ok(Expr::Not(Box::new(self.unary()?)));
        }
        if self.peek() == Some(Token::Tilde) {
            self.pos += 1;
            return Ok(Expr::BitNot(Box::new(self.unary()?)));
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr<'a>, Error<'a>> {
        match self.next() {
            Some(Token::Number(n)) => Ok(Expr::Literal(Value::Int64(n))),
            Some(Token::True) => Ok(Expr::Literal(Value::Boolean(true))),
            Some(Token::False) => Ok(Expr::Literal(Value::Boolean(false))),
            Some(Token::Name(name)) if self.peek() == Some(Token::LeftParen) => {
                self.pos += 1;
                let mut args = Vec::new();
                while self.peek() != Some(Token::RightParen) {
                    if !args.is_empty() {
                        self.expect(Token::Comma, "expected ',' or ')'")?;
                    }
                    args.push(self.expression()?);
                }
                self.pos += 1; // the ')'
                Ok(Expr::Call(name, args))
            },
            Some(Token::Name(_)) if matches!(self.peek(), Some(Token::PlusPlus | Token::MinusMinus)) => {
                Err(Error::Syntax("'++' and '--' only work as a statement"))
            },
            Some(Token::Name(name)) => Ok(Expr::Variable(name)),
            Some(Token::LeftParen) => {
                let inside = self.expression()?;
                if self.next() != Some(Token::RightParen) {
                    return Err(Error::Syntax("expected ')'"));
                }
                Ok(inside)
            }
            Some(Token::PlusPlus | Token::MinusMinus) => {
                Err(Error::Syntax("'++' and '--' only work as a statement"))
            },
            Some(_) => Err(Error::Syntax("expected a number, variable, or '('")),
            None => Err(Error::Syntax("unexpected end of input")),
        }
    }
}

fn eval<'a>(expr: &Expr<'a>, vars: &mut Env<'a, '_>) -> Result<Value, Error<'a>> {
    vars.check_stack()?;
    match expr {
        Expr::Literal(value) => Ok(*value),
        Expr::Variable(name) => {
            vars.get(*name).map(|variable| variable.value).ok_or(Error::UnknownName(name))
        },
        Expr::Negate(inner) => {
            let n = eval(inner, vars)?.int64("'-'")?;
            n.checked_neg().map(Value::Int64).ok_or(Error::Overflow)
        },
        Expr::Not(inner) => Ok(Value::Boolean(!eval(inner, vars)?.boolean("'!'")?)),
        Expr::BitNot(inner) => Ok(Value::Int64(!eval(inner, vars)?.int64("'~'")?)),
        Expr::And(left, right) => Ok(Value::Boolean(
            eval(left, vars)?.boolean("'&&'")? && eval(right, vars)?.boolean("'&&'")?,
        )),
        Expr::Or(left, right) => Ok(Value::Boolean(
            eval(left, vars)?.boolean("'||'")? || eval(right, vars)?.boolean("'||'")?,
        )),
        Expr::Binary(op, left, right) => {
            let a = eval(left, vars)?;
            let b = eval(right, vars)?;
            let place = op.symbol();
            let (a, b) = match (op, a, b) {
                (_, Value::Int64(a), Value::Int64(b)) => (a, b),
                (Op::Equal, Value::Boolean(a), Value::Boolean(b)) => {
                    return Ok(Value::Boolean(a == b));
                },
                (Op::NotEqual, Value::Boolean(a), Value::Boolean(b)) => {
                    return Ok(Value::Boolean(a != b));
                }
                (Op::BitAnd, Value::Boolean(a), Value::Boolean(b)) => return Ok(Value::Boolean(a & b)),
                (Op::BitOr, Value::Boolean(a), Value::Boolean(b)) => return Ok(Value::Boolean(a | b)),
                (Op::BitXor, Value::Boolean(a), Value::Boolean(b)) => return Ok(Value::Boolean(a ^ b)),
                (Op::Equal | Op::NotEqual | Op::BitAnd | Op::BitOr | Op::BitXor, a, b) => {
                    return Err(Error::WrongType {place, wanted: a.type_name(), found: b.type_name() });
                },
                _ => return Err(Error::WrongType {place, wanted: "Int64", found: "Boolean" }),
            };
            let result = match op {
                Op::Add => a.checked_add(b).map(Value::Int64),
                Op::Subtract => a.checked_sub(b).map(Value::Int64),
                Op::Multiply => a.checked_mul(b).map(Value::Int64),
                Op::Divide | Op::Remainder if b == 0 => return Err(Error::DivideByZero),
                Op::Divide => a.checked_div(b).map(Value::Int64),
                Op::Remainder => a.checked_rem(b).map(Value::Int64),
                Op::Equal => Some(Value::Boolean(a == b)),
                Op::NotEqual => Some(Value::Boolean(a != b)),
                Op::Less => Some(Value::Boolean(a < b)),
                Op::LessEqual => Some(Value::Boolean(a <= b)),
                Op::Greater => Some(Value::Boolean(a > b)),
                Op::GreaterEqual => Some(Value::Boolean(a >= b)),
                Op::BitAnd => Some(Value::Int64(a & b)),
                Op::BitOr => Some(Value::Int64(a | b)),
                Op::BitXor => Some(Value::Int64(a ^ b)),
                Op::ShiftLeft => Some(Value::Int64(a.wrapping_shl(b as u32))),
                Op::ShiftRight => Some(Value::Int64(a.wrapping_shr(b as u32))),
            };
            result.ok_or(Error::Overflow)
        },
        Expr::Call(name, args) => call(name, args, vars)?.ok_or(Error::NoValue(name)),
    }
}

const MAX_LOOP_ROUNDS: u64 = 1_000_000;

enum Flow {
    Normal(Option<Value>),  // statement reached the end
    Return(Option<Value>),  // statement was stopped by a `return`
    Break,                  // stopped by a `break` statement
    Continue,               // stopped by a `continue` statement
}

// Runs statements in order. Returns the value of the last item if that
// item is an expression or assignment.
fn run<'a>(stmts: &[Stmt<'a>], vars: &mut Env<'a, '_>) -> Result<Flow, Error<'a>> {
    let mut last = None;
    for stmt in stmts {
        last = match stmt {
            Stmt::Expr(Expr::Call(name, args)) => call(name, args, vars)?,
            Stmt::Expr(expr) => Some(eval(expr, vars)?),
            Stmt::Declare {name, ty, value, constant } => {
                let value = eval(value, vars)?;
                let ty = ty.unwrap_or(value.type_of());
                if value.type_of() != ty {
                    return Err(Error::VariableType { name, wanted: ty.name(), found: value.type_name() });
                }
                vars.declare(name, Variable { value, constant: *constant })?;
                Some(value)
            },
            Stmt::Assign(name, expr) => {
                let value = eval(expr, vars)?;
                let Some(variable) = vars.get_mut(*name) else {
                    return Err(Error::Undeclared(name));
                };
                if variable.constant {
                    return Err(Error::Constant(name));
                }
                if value.type_of() != variable.value.type_of() {
                    let wanted = variable.value.type_name();
                    return Err(Error::VariableType { name, wanted, found: value.type_name() });
                }
                variable.value = value;
                Some(value)
            },
            Stmt::If(condition, then_block, else_block) => {
                let block = if eval(condition, vars)?.boolean("if")? { then_block } else { else_block };
                match run_block(block, vars)? {
                    Flow::Normal(_) => None,
                    flow => return Ok(flow),
                }
            },
            Stmt::Loop { keyword, init, condition, step, body } => {
                let start = vars.locals.len();
                vars.depth += 1;
                run(init, vars)?;
                let mut rounds = 0;
                while eval(condition, vars)?.boolean(keyword)? {
                    rounds += 1;
                    if rounds > MAX_LOOP_ROUNDS {
                        return Err(Error::LoopLimit);
                    }
                    match run_block(body, vars)? {
                        Flow::Break => break,
                        Flow::Return(value) => {
                            vars.depth -= 1;
                            vars.locals.truncate(start);
                            return Ok(Flow::Return(value));
                        }
                        Flow::Normal(_) | Flow::Continue => {}
                    }
                    run(step, vars)?;
                }
                vars.depth -= 1;
                vars.locals.truncate(start);
                None
            }
            Stmt::Return(expr) => {
                let value = match expr {
                    Some(expr) => Some(eval(expr, vars)?),
                    None => None,
                };
                return Ok(Flow::Return(value));
            },
            Stmt::Break => return Ok(Flow::Break),
            Stmt::Continue => return Ok(Flow::Continue),
        }
    }
    Ok(Flow::Normal(last))
}

// Runs statements within a { } block.
fn run_block<'a>(stmts: &[Stmt<'a>], vars: &mut Env<'a, '_>) -> Result<Flow, Error<'a>> {
    let start = vars.locals.len();
    vars.depth += 1;
    let flow = run(stmts, vars)?;
    vars.depth -= 1;
    vars.locals.truncate(start);
    Ok(flow)
}

// Calls a function and returns the value it gives back (or None).
fn call<'a>(name: &'a str, args: &[Expr<'a>], vars: &mut Env<'a, '_>) -> Result<Option<Value>, Error<'a>> {
    // `functions` is a shared reference. We need a copy to use in case `vars` changes.
    let functions = vars.functions;
    let function = functions.get(name).ok_or(Error::UnknownFunction(name))?;
    if args.len() != function.params.len() {
        return Err(Error::ArgumentCount { name, wanted: function.params.len(), found: args.len() });
    }
    vars.check_stack()?;
    let mut locals: Vec<(&'a str, Variable)> = Vec::new();
    for ((param, ty), arg) in function.params.iter().zip(args) {
        let value = eval(arg, vars)?;
        if value.type_of() != *ty {
            return Err(Error::VariableType { name: param, wanted: ty.name(), found: value.type_name() });
        }
        locals.push((param, Variable { value, constant: false }));
    }
    let caller = core::mem::replace(&mut vars.locals, locals);
    let flow = run_block(&function.body, vars)?;
    vars.locals = caller;
    let value = match flow {
        Flow::Return(value) => value,
        Flow::Normal(_) | Flow::Break | Flow::Continue => None,
    };
    match (value, function.returns) {
        (Some(value), Some(returns)) if value.type_of() == returns => Ok(Some(value)),
        (None, None) => Ok(None),
        (None, Some(_)) => Err(Error::NoReturn(name)),
        (Some(value), returns) => {
            let wanted = returns.map_or("Void", Type::name);
            Err(Error::VariableType { name, wanted, found: value.type_name() })
        }
    }
}

// Turns a line into its component statements.
fn parse<'a>(
    text: &'a str,
    vars: &Variables,
) -> Result<(Vec<Stmt<'a>>, Vec<(&'a str, Function<'a>)>), Error<'a>> {
    let tokens = tokenize(text)?;
    let mut parser = Parser { tokens, pos: 0, depth: 0, in_function: false, loops:0, functions: Vec::new() };
    let program = parser.statements().map_err(|error| match parser.tokens[..] {
        // A line that doesn't parse, and starts with a name that isn't a
        // variable or being assigned, is most likely mistyped.
        [Token::Name(name), second, ..]
            if !matches!(second, Token::Equals | Token::LeftParen) && !vars.contains_key(name) =>
            {
                Error::UnknownName(name)
            }
        _ => error,
    })?;
    if parser.pos < parser.tokens.len() {
        return Err(Error::Syntax("unexpected '}'"));
    }
    Ok((program, parser.functions))
}

// Runs a line. Returns the value to print, if any.
pub fn evaluate<'a>(
    text: &'a str,
    vars: &mut Variables,
    functions: &mut Functions,
) -> Result<Option<Value>, Error<'a>> {
    let (mut program, declared) = parse(text, vars)?;
    if !declared.is_empty() {
        let text: &'static str = Box::leak(String::from(text).into_boxed_str());
        let (forever, declared) = parse(text, vars)?;
        for (name, function) in declared {
            functions.insert(String::from(name), function);
        }
        program = forever;
    }
    let stack_start = stack_address();
    let mut env = Env { globals: vars, functions, locals: Vec::new(), depth: 0, stack_start };
    match run(&program, &mut env)? {
        Flow::Normal(value) | Flow::Return(value) => Ok(value),
        Flow::Break | Flow::Continue => Ok(None),
    }
}
