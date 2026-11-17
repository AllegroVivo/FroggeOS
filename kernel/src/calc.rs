// A calculator for integer expressions.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
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
    LoopLimit,              // a `while` loop repeated ``MAX_LOOP_ROUNDS`` times.
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
            '=' | '!' | '<' | '>' => {
                // NOTE: ``==``, ``!=``, ``<=`` and ``>=`` are two characters in one token.
                let equals_next = chars.next_if(|&(_, d)| d == '=').is_some();
                match (c, equals_next) {
                    ('=', false) => Token::Equals,
                    ('=', true) => Token::EqualEqual,
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

enum Expr<'a> {
    Number(i64),
    Variable(&'a str),
    Negate(Box<Expr<'a>>),
    Binary(Op, Box<Expr<'a>>, Box<Expr<'a>>),
}

enum Stmt<'a> {
    Expr(Expr<'a>),
    Assign(&'a str, Expr<'a>),
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
            (Some(Token::Name(name)), Some(Token::Equals)) => {
                self.pos += 2;
                Ok(Stmt::Assign(name, self.expression()?))
            },
            _ => Ok(Stmt::Expr(self.expression()?))
        }
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
        let mut left = self.sum()?;
        loop {
            let op = match self.peek() {
                Some(Token::EqualEqual) => Op::Equal,
                Some(Token::NotEqual) => Op::NotEqual,
                Some(Token::Less) => Op::Less,
                Some(Token::LessEqual) => Op::LessEqual,
                Some(Token::Greater) => Op::Greater,
                Some(Token::GreaterEqual) => Op::GreaterEqual,
                _ => return Ok(left),
            };
            self.pos += 1;
            let right = self.sum()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
    }

    fn sum(&mut self) -> Result<Expr<'a>, Error<'a>> {
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
                Op::Equal => Some((a == b) as i64),
                Op::NotEqual => Some((a != b) as i64),
                Op::Less => Some((a < b) as i64),
                Op::LessEqual => Some((a <= b) as i64),
                Op::Greater => Some((a > b) as i64),
                Op::GreaterEqual => Some((a >= b) as i64),
            };
            result.ok_or(Error::Overflow)
        }
    }
}

const MAX_LOOP_ROUNDS: u64 = 1_000_000;

// Runs statements in order. Returns the value of the last item if that
// item is an expression or assignment.
fn run<'a>(stmts: &[Stmt<'a>], vars: &mut Variables) -> Result<Option<i64>, Error<'a>> {
    let mut last = None;
    for stmt in stmts {
        last = match stmt {
            Stmt::Expr(expr) => Some(eval(expr, vars)?),
            Stmt::Assign(name, expr) => {
                let value = eval(expr, vars)?;
                match vars.get_mut(*name) {
                    Some(slot) => *slot = value,
                    None => {
                        vars.insert(String::from(*name), value);
                    }
                }
                Some(value)
            },
            Stmt::If(condition, then_block, else_block) => {
                if eval(condition, vars)? != 0 {
                    run(then_block, vars)?;
                } else {
                    run (else_block, vars)?;
                }
                None
            },
            Stmt::While(condition, body) => {
                let mut rounds = 0;
                while eval(condition, vars)? != 0 {
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
pub fn evaluate<'a>(text: &'a str, vars: &mut Variables) -> Result<Option<i64>, Error<'a>> {
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
