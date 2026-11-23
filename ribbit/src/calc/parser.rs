// Reads tokens into a tree.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use super::error::Error;
use super::token::{Op, Token, tokenize};
use super::tree::{Block, BuiltIn, Expr, Stmt};
use super::value::Value;
use super::{Function, MAX_STACK, stack_address};

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

fn shorthand<'a>(at: usize, name: &'a str, op_at: usize, op: Op, value: Expr<'a>) -> Stmt<'a> {
    Stmt::Assign(name, Expr::Binary(op_at, op, Box::new(Expr::Variable(at, name)), Box::new(value)))
}

// A table outlining the valid string-escape patterns.
fn escaped<'a>(chars: &mut impl Iterator<Item = char>) -> Result<char, Error<'a>> {
    match chars.next() {
        Some('n') => Ok('\n'),
        Some('t') => Ok('\t'),
        Some('\\') => Ok('\\'),
        Some('"') => Ok('"'),
        Some('\'') => Ok('\''),
        Some('0') => Ok('\0'),
        Some('x') => {
            let digits = [chars.next(), chars.next()].map(|d| d.and_then(|d| d.to_digit(16)));
            let [Some(high), Some(low)] = digits else {
                return Err(Error::Syntax("'\\x' needs two hex digits"));
            };
            match high * 16 + low {
                code @ 0x00..=0x7F => Ok(code as u8 as char),
                _ => Err(Error::Syntax("only supports '\\x00' to '\\x7F'")),
            }
        },
        _ => Err(Error::Syntax("unknown escape sequence")),
    }
}

// The text a string literal stands for. (e.g. '\n' -> 'new line')
fn unescape<'a>(raw: &str) -> Result<Rc<String>, Error<'a>> {
    let mut text = String::new();
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        text.push(if c == '\\' { escaped(&mut chars)? } else { c });
    }
    Ok(Rc::new(text))
}

// A character literal, possibly escaped.
#[inline(never)]
fn character<'a>(raw: &str) -> Result<Expr<'a>, Error<'a>> {
    match unescape(raw)?.as_bytes() {
        [byte] => Ok(Expr::Literal(Value::Char(*byte))),
        _ => Err(Error::Syntax("a Char may only hold one character")),
    }
}

// Interpolation shorthand helper
fn join<'a>(at: usize, a: Expr<'a>, b: Expr<'a>) -> Expr<'a> {
    Expr::Binary(at, Op::Add, Box::new(a), Box::new(b))
}

// Functionally breaks string interpolations ($"x = {x}!") into their components.
#[inline(never)]
#[allow(clippy::needless_lifetimes)]
fn interpolation<'a>(raw: &'a str, at: usize, stack_start: usize) -> Result<Expr<'a>, Error<'a>> {
    let mut joined = Expr::Literal(Value::String(Rc::new(String::new())));
    let mut text = String::new();
    let mut chars = raw.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        match c {
            '{' if chars.next_if(|&(_, d)| d == '{').is_some() => text.push('{'),
            '}' if chars.next_if(|&(_, d)| d == '}').is_some() => text.push('}'),
            '{' => {
                let end = loop {
                    match chars.next() {
                        Some((end, '}')) => break end,
                        Some(_) => {},
                        None => return Err(Error::Syntax("missing closing '}' for string interpolation. escape brackets with '{{'"))
                    }
                };
                if !text.is_empty() {
                    let piece = core::mem::take(&mut text);
                    joined = join(at, joined, Expr::Literal(Value::String(Rc::new(piece))));
                }
                joined = join(at, joined, hole(&raw[start + 1..end], at + start + 1, stack_start)?);
            },
            '}' => return Err(Error::Syntax("missing opening '{' for string interpolation. escape brackets with '}}'")),
            '\\' => text.push(escaped(&mut chars.by_ref().map(|(_, d)| d))?),
            c => text.push(c),
        }
    }
    if !text.is_empty() {
        joined = join(at, joined, Expr::Literal(Value::String(Rc::new(text))));
    }
    Ok(joined)
}

// A single interpolate {hole} in a string.
#[allow(clippy::needless_lifetimes)]
fn hole<'a>(inside: &'a str, at: usize, stack_start: usize) -> Result<Expr<'a>, Error<'a>> {
    let (code, format) = match inside.split_once(':') {
        Some((code, format)) => (code, Some(format)),
        None => (inside, None),
    };
    let (tokens, spans) = tokenize(code).map_err(|(error, _)| error)?;
    let spans = spans.into_iter().map(|(start, end)| (start + at, end + at)).collect();
    let mut parser = Parser::new(tokens, spans, stack_start);
    let value = parser.expression()?;
    if parser.pos < parser.tokens.len() {
        return Err(Error::Syntax("an interpolate {hole} may only hold one expression"));
    }
    let Some(format) = format else {
        return Ok(value);
    };
    let bad = Error::Syntax("an interpolation format must be in the style of {n:x8}");
    let upper = match format.chars().next() {
        Some('X') => true,
        Some('x') => false,
        _ => return Err(bad),
    };
    let width = match &format[1..] {
        "" => 0,
        digits => digits.parse::<u8>().map_err(|_| bad)?,
    };
    Ok(Expr::Hex(at, Box::new(value), upper, width))
}

pub(super) struct Parser<'a> {
    pub(super) tokens: Vec<Token<'a>>,
    pub(super) spans: Vec<(usize, usize)>,
    pub(super) pos: usize,
    looked: usize,
    missing: bool,
    depth: usize,
    in_function: bool,
    loops: usize,
    stack_start: usize,
    pub(super) functions: Vec<(&'a str, Function<'a>)>,
}

impl<'a> Parser<'a> {

    pub(super) fn new(tokens: Vec<Token<'a>>, spans: Vec<(usize, usize)>, stack_start: usize) -> Parser<'a> {
        Parser {
            tokens, spans, pos: 0, looked: 0, missing: false,
            depth: 0, in_function: false, loops: 0, stack_start, functions: Vec::new()
        }
    }

    fn peek(&mut self) -> Option<Token<'a>> {
        self.looked = self.pos;
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
            self.missing = true;
            return Err(Error::Syntax(message));
        }
        Ok(())
    }

    pub(super) fn position(&self) -> usize {
        let looked = self.looked.min(self.spans.len());
        let after_previous = if looked > 0 { self.spans[looked - 1].1 } else { 0 };
        match self.spans.get(looked) {
            Some(&(start, _)) if !self.missing => start,
            _ => after_previous,
        }
    }

    // Where the next token starts.
    fn here(&self) -> usize {
        self.spans.get(self.pos).map_or(0, |&(start, _)| start)
    }

    // Stops a line that nests deeply enough to exceed `MAX_STACK`.
    fn check_stack(&self) -> Result<(), Error<'a>> {
        if self.stack_start.saturating_sub(stack_address()) > MAX_STACK {
            return Err(Error::TooDeep);
        }
        Ok(())
    }

    // A whole line, or the inside of a block. Statements must end with `;` except
    // in an `if`, `while`, or `for`, as they already end with a closing brace.
    pub(super) fn statements(&mut self) -> Result<Block<'a>, Error<'a>> {
        let mut list = Vec::new();
        loop {
            match (self.peek(), self.peek_at(1), self.peek_at(2)) {
                (None | Some(Token::RightBrace), _, _) => return Ok(list),
                (Some(Token::Semicolon), _, _) => self.pos += 1,
                (Some(Token::Type(_) | Token::Void), Some(Token::Name(_)), Some(Token::LeftParen)) => {
                    self.function()?;
                },
                _ => {
                    let at = self.here();
                    let stmt = self.statement()?;
                    let ends_with_block = matches!(stmt, Stmt::If(..) | Stmt::Loop { body_first: false, .. });
                    list.push((at, stmt));
                    if !ends_with_block {
                        self.expect(Token::Semicolon, "expected ';'")?;
                    }
                }
            }
        }
    }

    fn statement(&mut self) -> Result<Stmt<'a>, Error<'a>> {
        self.check_stack()?;
        // Look at the next two tokens. `=` starts an assignment.
        match (self.peek(), self.tokens.get(self.pos + 1).copied()) {
            (Some(Token::If), _) => {
                self.pos += 1;
                let condition = self.condition()?;
                let then_block = self.block()?;
                let else_block = if self.peek() == Some(Token::Else) {
                    self.pos += 1;
                    if self.peek() == Some(Token::If) {
                        vec![(self.here(), self.statement()?)] // (else if)
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
                Ok(Stmt::Loop { keyword: "while", init: Vec::new(), condition, step: Vec::new(), body, body_first: false })
            },
            (Some(Token::Do), _) => {
                self.pos += 1;
                let body = self.loop_body()?;
                self.expect(Token::While, "expected 'while'")?;
                let condition = self.condition()?;
                Ok(Stmt::Loop { keyword: "do", init: Vec::new(), condition, step: Vec::new(), body, body_first: true })
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
                Ok(Stmt::Loop { keyword: "for", init, condition, step, body, body_first: false })
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
            (Some(Token::Assert), _) => {
                self.pos += 1;
                let condition = self.expression()?;
                let message = if self.peek() == Some(Token::Comma) {
                    self.pos += 1;
                    Some(self.expression()?)
                } else {
                    None
                };
                Ok(Stmt::Assert(condition, message))
            },
            (Some(Token::Void), _) => Err(Error::Syntax("cannot assign void to a variable")),
            (Some(Token::Type(_) | Token::Var | Token::Const), _) => self.declaration(),
            (Some(Token::Name(name)), Some(Token::Equals)) => {
                self.pos += 2;
                Ok(Stmt::Assign(name, self.expression()?))
            },
            (Some(Token::Name(name)), Some(Token::Compound(op))) => {
                let (at, op_at) = (self.here(), self.spans[self.pos + 1].0);
                self.pos += 2;
                Ok(shorthand(at, name, op_at, op, self.expression()?))
            },
            (Some(Token::Name(name)), Some(token @ (Token::PlusPlus | Token::MinusMinus))) => {
                let (at, op_at) = (self.here(), self.spans[self.pos + 1].0);
                self.pos += 2;
                let op = if token == Token::PlusPlus { Op::Add } else { Op::Subtract };
                Ok(shorthand(at, name, op_at, op, Expr::Literal(Value::Int64(1))))
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
            (_, Some(ty)) if !constant => match ty.zero() {
                Some(zero) => Expr::Literal(zero),
                None => return Err(Error::Syntax("a String must be assigned a value")),
            },
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
    fn loop_body(&mut self) -> Result<Block<'a>, Error<'a>> {
        self.loops += 1;
        let body = self.block()?;
        self.loops -= 1;
        Ok(body)
    }

    fn for_part(&mut self, end: Token<'a>, message: &'static str) -> Result<Block<'a>, Error<'a>> {
        let part = if self.peek() == Some(end) { Vec::new() } else { vec![(self.here(), self.statement()?)] };
        self.expect(end, message)?;
        Ok(part)
    }
    
    fn block(&mut self) -> Result<Block<'a>, Error<'a>> {
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
            let at = self.here();
            self.pos += 1;
            // Right side only takes operators that bind tighter.
            let right = self.binary(precedence + 1)?;
            let (left_side, right) = (Box::new(left), Box::new(right));
            left = match infix {
                Infix::Op(op) => Expr::Binary(at, op, left_side, right),
                Infix::And => Expr::And(at, left_side, right),
                Infix::Or => Expr::Or(at, left_side, right),
            };
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Expr<'a>, Error<'a>> {
        self.check_stack()?;
        let at = self.here();
        if self.peek() == Some(Token::Minus) {
            self.pos += 1;
            return Ok(Expr::Negate(at, Box::new(self.unary()?)));
        }
        if self.peek() == Some(Token::Bang) {
            self.pos += 1;
            return Ok(Expr::Not(at, Box::new(self.unary()?)));
        }
        if self.peek() == Some(Token::Tilde) {
            self.pos += 1;
            return Ok(Expr::BitNot(at, Box::new(self.unary()?)));
        }
        let expr = self.primary()?;
        self.postfix(expr)
    }

    #[inline(never)]
    fn postfix(&mut self, mut expr: Expr<'a>) -> Result<Expr<'a>, Error<'a>> {
        loop {
            match self.peek() {
                Some(Token::LeftBracket) => {
                    let at = self.here();
                    self.pos += 1;
                    let index = self.expression()?;
                    self.expect(Token::RightBracket, "expected ']'")?;
                    expr = Expr::Index(at, Box::new(expr), Box::new(index));
                },
                Some(Token::Dot) => {
                    self.pos += 1;
                    let at = self.here();
                    let Some(Token::Name(name)) = self.next() else {
                        return Err(Error::Syntax("expected a name after '.'"));
                    };
                    expr = Expr::Member(at, Box::new(expr), name);
                },
                _ => return Ok(expr),
            }
        }
    }

    // Determines whether the identifier following a '.' calls a builtin.
    #[inline(never)]
    fn builtin_follows(&self, namespace: &str) -> bool {
        match (self.peek_at(0), self.peek_at(1), self.peek_at(2)) {
            (Some(Token::Dot), Some(Token::Name(name)), next) => {
                next == Some(Token::LeftParen) || BuiltIn::find(namespace, name).is_some()
            },
            _ => false,
        }
    }

    fn primary(&mut self) -> Result<Expr<'a>, Error<'a>> {
        let at = self.here();
        match self.next() {
            Some(Token::Number(n)) => Ok(Expr::Literal(Value::Int64(n))),
            Some(Token::True) => Ok(Expr::Literal(Value::Boolean(true))),
            Some(Token::False) => Ok(Expr::Literal(Value::Boolean(false))),
            Some(Token::Char(raw)) => character(raw),
            Some(Token::Str(raw)) => Ok(Expr::Literal(Value::String(unescape(raw)?))),
            Some(Token::Interp(raw)) => interpolation(raw, at + 2, self.stack_start),
            Some(Token::Name(name)) if self.peek() == Some(Token::LeftParen) => {
                Ok(Expr::Call(at, name, self.arguments()?))
            },
            Some(Token::Name(namespace)) if self.builtin_follows(namespace) => {
                self.pos += 1;
                let Some(Token::Name(name)) = self.next() else {
                    return Err(Error::Syntax("expected a name after '.'"));
                };
                let Some(builtin) = BuiltIn::find(namespace, name) else {
                    return Err(Error::UnknownBuiltIn(namespace, name));
                };
                Ok(Expr::BuiltIn(at, builtin, self.arguments()?))
            },
            Some(Token::Name(_)) if matches!(self.peek(), Some(Token::PlusPlus | Token::MinusMinus)) => {
                Err(Error::Syntax("'++' and '--' only work as a statement"))
            },
            Some(Token::Name(name)) => Ok(Expr::Variable(at, name)),
            Some(Token::LeftParen) => {
                let inside = self.expression()?;
                self.expect(Token::RightParen, "expected ')'")?;
                Ok(inside)
            }
            Some(Token::PlusPlus | Token::MinusMinus) => {
                Err(Error::Syntax("'++' and '--' only work as a statement"))
            },
            Some(_) => Err(Error::Syntax("expected a number, variable, or '('")),
            None => Err(Error::Syntax("unexpected end of input")),
        }
    }

    // The arguments of a call.
    fn arguments(&mut self) -> Result<Vec<Expr<'a>>, Error<'a>> {
        self.expect(Token::LeftParen, "expected '('")?;
        let mut args = Vec::new();
        while self.peek() != Some(Token::RightParen) {
            if !args.is_empty() {
                self.expect(Token::Comma, "expected ',' or ')'")?;
            }
            args.push(self.expression()?);
        }
        self.pos += 1;
        Ok(args)
    }
}
