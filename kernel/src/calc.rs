// A calculator for integer expressions.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt;
// ---- Errors ----

pub enum Error {
    BadChar(char),          // a character that isn't part of any token
    Syntax(&'static str),   // the tokens don't form an expression
    DivideByZero,
    Overflow,               // a number or result is too big for an i64
}

// Display is used by ``println!("{}", error)``
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::BadChar(c) => write!(f, "unexpected character '{}'", c),
            Error::Syntax(message) => write!(f, "{}", message),
            Error::DivideByZero => write!(f, "division by zero"),
            Error::Overflow => write!(f, "number too big")
        }
    }
}

// ``derive`` instructs the compiler to write these traits automatically.
// ``Copy`` allows a token to be copied in the same manner as an integer.
// ``PartialEq`` allows for comparison with ``==``.
#[derive(Clone, Copy, PartialEq)]
enum Token {
    Number(i64),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    LeftParen,
    RightParen,
}

fn tokenize(text: &str) -> Result<Vec<Token>, Error> {
    let mut tokens = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        let token = match c {
            ' ' => continue,
            '+' => Token::Plus,
            '-' => Token::Minus,
            '*' => Token::Star,
            '/' => Token::Slash,
            '%' => Token::Percent,
            '(' => Token::LeftParen,
            ')' => Token::RightParen,
            '0'..='9' => {
                let mut value = (c as u8 - b'0') as i64;
                while let Some(&d) = chars.peek() {
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

enum Expr {
    Number(i64),
    Negate(Box<Expr>),
    Binary(Op, Box<Expr>, Box<Expr>),
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<Token> {
        self.tokens.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.peek();
        self.pos += 1;
        token
    }

    fn expression(&mut self) -> Result<Expr, Error> {
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

    fn term(&mut self) -> Result<Expr, Error> {
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

    fn unary(&mut self) -> Result<Expr, Error> {
        if self.peek() == Some(Token::Minus) {
            self.pos += 1;
            return Ok(Expr::Negate(Box::new(self.unary()?)));
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr, Error> {
        match self.next() {
            Some(Token::Number(n)) => Ok(Expr::Number(n)),
            Some(Token::LeftParen) => {
                let inside = self.expression()?;
                if self.next() != Some(Token::RightParen) {
                    return Err(Error::Syntax("expected ')'"));
                }
                Ok(inside)
            }
            Some(_) => Err(Error::Syntax("expected a number or '('")),
            None => Err(Error::Syntax("unexpected end of input")),
        }
    }
}

fn eval(expr: &Expr) -> Result<i64, Error> {
    match expr {
        Expr::Number(n) => Ok(*n),
        Expr::Negate(inner) => eval(inner)?.checked_neg().ok_or(Error::Overflow),
        Expr::Binary(op, left, right) => {
            let a = eval(left)?;
            let b = eval(right)?;
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

pub fn evaluate(text: &str) -> Result<i64, Error> {
    let tokens = tokenize(text)?;
    let mut parser = Parser { tokens, pos: 0 };
    let expr = parser.expression()?;
    if parser.pos < parser.tokens.len() {
        return Err(Error::Syntax("unexpected input after the expression"));
    }
    eval(&expr)
}
