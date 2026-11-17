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

pub enum Error<'a> {
    BadChar(char),          // a character that isn't part of any token
    Syntax(&'static str),   // the tokens don't form an expression
    DivideByZero,
    Overflow,               // a number or result is too big for an i64
    UnknownName(&'a str),   // a name with no value stored under it
    LoopLimit,              // a `while` loop repeated ``MAX_LOOP_ROUNDS`` times
    // A value of the wrong type.
    WrongType { place: &'static str, wanted: &'static str, found: &'static str},
    Undeclared(&'a str),    // assigning to a name that was not declared
    Constant(&'a str),      // assigning to a const
    // Attempting to store the wrong type into a variable.
    VariableType { name: &'a str, wanted: &'static str, found: &'static str },
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
            Error::VariableType { name, wanted, found } => {
                write!(f, "'{}' needs {}, not {}", name, wanted, found)
            },
        }
    }
}

// ``derive`` instructs the compiler to write these traits automatically.
// ``Copy`` allows a token to be copied in the same manner as an integer.
// ``PartialEq`` allows for comparison with ``==``.
#[derive(Clone, Copy, PartialEq)]
enum Token<'a> {
    Number(i64),
    Name(&'a str),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    LeftParen,
    RightParen,
    Equals,
    EqualEqual,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Semicolon,
    LeftBrace,
    RightBrace,
    If,
    Else,
    While,
    Bang,
    AndAnd,
    OrOr,
    True,
    False,
    Var,
    Const,
    Type(Type),
}

#[allow(clippy::needless_lifetimes)]
fn tokenize<'a>(text: &'a str) -> Result<Vec<Token<'a>>, Error<'a>> {
    let mut tokens = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        let token = match c {
            ' ' => continue,
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
            '&' if chars.next_if(|&(_, d)| d == '&').is_some() => Token::AndAnd,
            '|' if chars.next_if(|&(_, d)| d == '|').is_some() => Token::OrOr,
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
                    name => Token::Name(name),
                }
            }
            other => return Err(Error::BadChar(other))
        };
        tokens.push(token);
    }
    Ok(tokens)
}

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
        Token::EqualEqual => (3, Infix::Op(Op::Equal)),
        Token::NotEqual => (3, Infix::Op(Op::NotEqual)),
        Token::Less => (4, Infix::Op(Op::Less)),
        Token::LessEqual => (4, Infix::Op(Op::LessEqual)),
        Token::Greater => (4, Infix::Op(Op::Greater)),
        Token::GreaterEqual => (4, Infix::Op(Op::GreaterEqual)),
        Token::Plus => (5, Infix::Op(Op::Add)),
        Token::Minus => (5, Infix::Op(Op::Subtract)),
        Token::Star => (6, Infix::Op(Op::Multiply)),
        Token::Slash => (6, Infix::Op(Op::Divide)),
        Token::Percent => (6, Infix::Op(Op::Remainder)),
        _ => return None,
    })
}

enum Expr<'a> {
    Literal(Value),
    Variable(&'a str),
    Negate(Box<Expr<'a>>),
    Not(Box<Expr<'a>>),
    Binary(Op, Box<Expr<'a>>, Box<Expr<'a>>),
    And(Box<Expr<'a>>, Box<Expr<'a>>),
    Or(Box<Expr<'a>>, Box<Expr<'a>>),
}

enum Stmt<'a> {
    Expr(Expr<'a>),
    Assign(&'a str, Expr<'a>),
    Declare { name: &'a str, ty: Option<Type>, value: Expr<'a>, constant: bool },
    If(Expr<'a>, Vec<Stmt<'a>>, Vec<Stmt<'a>>), // condition, then, else
    While(Expr<'a>, Vec<Stmt<'a>>),             // condition, body
}

struct Parser<'a> {
    tokens: Vec<Token<'a>>,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<Token<'a>> {
        self.tokens.get(self.pos).copied()
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
            match self.peek() {
                None | Some(Token::RightBrace) => return Ok(list),
                Some(Token::Semicolon) => self.pos += 1,
                _ => {
                    let stmt = self.statement()?;
                    let ends_with_block = matches!(stmt, Stmt::If(..) | Stmt::While(..));
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
                let body = self.block()?;
                Ok(Stmt::While(condition, body))
            },
            (Some(Token::Type(_) | Token::Var | Token::Const), _) => self.declaration(),
            (Some(Token::Name(name)), Some(Token::Equals)) => {
                self.pos += 2;
                Ok(Stmt::Assign(name, self.expression()?))
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

    // The condition of an ``if`` or ``while``.
    fn condition(&mut self) -> Result<Expr<'a>, Error<'a>> {
        self.expect(Token::LeftParen, "expected '(' after if or while")?;
        let condition = self.expression()?;
        self.expect(Token::RightParen, "expected ')'")?;
        Ok(condition)
    }
    
    fn block(&mut self) -> Result<Vec<Stmt<'a>>, Error<'a>> {
        self.expect(Token::LeftBrace, "expected '{'")?;
        let body = self.statements()?;
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
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr<'a>, Error<'a>> {
        match self.next() {
            Some(Token::Number(n)) => Ok(Expr::Literal(Value::Int64(n))),
            Some(Token::True) => Ok(Expr::Literal(Value::Boolean(true))),
            Some(Token::False) => Ok(Expr::Literal(Value::Boolean(false))),
            Some(Token::Name(name)) => Ok(Expr::Variable(name)),
            Some(Token::LeftParen) => {
                let inside = self.expression()?;
                if self.next() != Some(Token::RightParen) {
                    return Err(Error::Syntax("expected ')'"));
                }
                Ok(inside)
            }
            Some(_) => Err(Error::Syntax("expected a number, variable, or '('")),
            None => Err(Error::Syntax("unexpected end of input")),
        }
    }
}

fn eval<'a>(expr: &Expr<'a>, vars: &Variables) -> Result<Value, Error<'a>> {
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
                (Op::Equal | Op::NotEqual, a, b) => {
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
            };
            result.ok_or(Error::Overflow)
        },
    }
}

const MAX_LOOP_ROUNDS: u64 = 1_000_000;

// Runs statements in order. Returns the value of the last item if that
// item is an expression or assignment.
fn run<'a>(stmts: &[Stmt<'a>], vars: &mut Variables) -> Result<Option<Value>, Error<'a>> {
    let mut last = None;
    for stmt in stmts {
        last = match stmt {
            Stmt::Expr(expr) => Some(eval(expr, vars)?),
            Stmt::Declare {name, ty, value, constant } => {
                let value = eval(value, vars)?;
                let ty = ty.unwrap_or(value.type_of());
                if value.type_of() != ty {
                    return Err(Error::VariableType { name, wanted: ty.name(), found: value.type_name() });
                }
                vars.insert(String::from(*name), Variable { value, constant: *constant });
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
                if eval(condition, vars)?.boolean("if")? {
                    run(then_block, vars)?;
                } else {
                    run (else_block, vars)?;
                }
                None
            },
            Stmt::While(condition, body) => {
                let mut rounds = 0;
                while eval(condition, vars)?.boolean("while")? {
                    rounds += 1;
                    if rounds > MAX_LOOP_ROUNDS {
                        return Err(Error::LoopLimit);
                    }
                    run(body, vars)?;
                }
                None
            }
        }
    }
    Ok(last)
}

// Runs a line. Returns the value to print, if any.
pub fn evaluate<'a>(text: &'a str, vars: &mut Variables) -> Result<Option<Value>, Error<'a>> {
    let tokens = tokenize(text)?;
    let mut parser = Parser { tokens, pos: 0 };
    let program = parser.statements().map_err(|error| match parser.tokens[..] {
        // A line that doesn't parse, and starts with a name that isn't a
        // variable or being assigned, is most likely a mistyped command.
        [Token::Name(name), second, ..]
            if second != Token::Equals && !vars.contains_key(name) =>
        {
            Error::UnknownName(name)
        }
        _ => error,
    })?;
    if parser.pos < parser.tokens.len() {
        return Err(Error::Syntax("unexpected '}'"));
    }
    run(&program, vars)
}
