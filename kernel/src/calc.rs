// A calculator for integer expressions.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

// Values stored from ``name = expression``.
pub type Variables = BTreeMap<String, i64>;

pub enum Error<'a> {
    BadChar(char),          // a character that isn't part of any token
    Syntax(&'static str),   // the tokens don't form an expression
    DivideByZero,
    Overflow,               // a number or result is too big for an i64
    UnknownName(&'a str),   // a name with no value stored under it
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
}

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
            '=' => Token::Equals,
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
                Token::Name(&text[start..end])
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
}

enum Expr<'a> {
    Number(i64),
    Variable(&'a str),
    Negate(Box<Expr<'a>>),
    Binary(Op, Box<Expr<'a>>, Box<Expr<'a>>),
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

    fn expression(&mut self) -> Result<Expr<'a>, Error<'a>> {
        let mut left = self.term()?;
        loop {
            let op = match self.peek() {
                Some(Token::Plus) => Op::Add,
                Some(Token::Minus) => Op::Subtract,
                _ => return Ok(left),
            };
            self.pos += 1;
            let right = self.term()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
    }

    fn term(&mut self) -> Result<Expr<'a>, Error<'a>> {
        let mut left = self.unary()?;
        loop {
            let op = match self.peek() {
                Some(Token::Star) => Op::Multiply,
                Some(Token::Slash) => Op::Divide,
                Some(Token::Percent) => Op::Remainder,
                _ => return Ok(left),
            };
            self.pos += 1;
            let right = self.unary()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
    }

    fn unary(&mut self) -> Result<Expr<'a>, Error<'a>> {
        if self.peek() == Some(Token::Minus) {
            self.pos += 1;
            return Ok(Expr::Negate(Box::new(self.unary()?)));
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr<'a>, Error<'a>> {
        match self.next() {
            Some(Token::Number(n)) => Ok(Expr::Number(n)),
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

fn eval<'a>(expr: &Expr<'a>, vars: &Variables) -> Result<i64, Error<'a>> {
    match expr {
        Expr::Number(n) => Ok(*n),
        Expr::Variable(name) => vars.get(*name).copied().ok_or(Error::UnknownName(name)),
        Expr::Negate(inner) => eval(inner, vars)?.checked_neg().ok_or(Error::Overflow),
        Expr::Binary(op, left, right) => {
            let a = eval(left, vars)?;
            let b = eval(right, vars)?;
            let result = match op {
                Op::Add => a.checked_add(b),
                Op::Subtract => a.checked_sub(b),
                Op::Multiply => a.checked_mul(b),
                Op::Divide | Op::Remainder if b == 0 => return Err(Error::DivideByZero),
                Op::Divide => a.checked_div(b),
                Op::Remainder => a.checked_rem(b),
            };
            result.ok_or(Error::Overflow)
        }
    }
}

pub fn evaluate<'a>(text: &'a str, vars: &mut Variables) -> Result<i64, Error<'a>> {
    let tokens = tokenize(text)?;
    let (target, start) = match tokens[..] {
        [Token::Name(name), Token::Equals, ..] => (Some(name), 2),
        _ => (None, 0),
    };
    let mut parser = Parser { tokens, pos: start };
    let expr = parser.expression()?;
    let value = eval(&expr, vars)?;
    if parser.pos < parser.tokens.len() {
        return Err(Error::Syntax("unexpected input after expression"));
    }
    if let Some(name) = target {
        vars.insert(String::from(name), value);
    }
    Ok(value)
}
