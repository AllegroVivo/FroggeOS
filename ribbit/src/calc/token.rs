// Splits a line into tokens.

use alloc::vec::Vec;
use core::iter::Peekable;
use core::str::CharIndices;
use crate::calc::token::Token::{LeftBracket, RightBracket};
use super::error::Error;
use super::value::Type;

// ``derive`` instructs the compiler to write these traits automatically.
// ``Copy`` allows a token to be copied in the same manner as an integer.
// ``PartialEq`` allows for comparison with ``==``.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Token<'a> {
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
    Do,                 // do
    Assert,             // assert
    Compound(Op),       // +=
    PlusPlus,           // ++
    MinusMinus,         // --
    Amp,                // &
    Pipe,               // |
    Caret,              // ^
    Tilde,              // ~
    ShiftLeft,          // <<
    ShiftRight,         // >>
    Str(&'a str),       // "Ribbit"
    Dot,                // .
    Interp(&'a str),    // $"x = {x}"
    Char(&'a str),      // 'R'
    LeftBracket,        // [
    RightBracket,       // ]
}

// Finds and returns the position of the " that ends a string.
fn closing_quote<'a>(chars: &mut Peekable<CharIndices<'_>>, quote: char) -> Result<usize, Error<'a>> {
    loop {
        match chars.next() {
            Some((end, c)) if c == quote => return Ok(end),
            Some((_, '\\')) => { chars.next(); },
            Some(_) => {},
            None if quote == '"' => return Err(Error::Syntax("missing a closing \"")),
            None => return Err(Error::Syntax("missing a closing '")),
        }
    }
}

#[allow(clippy::needless_lifetimes)]
pub(super) fn tokenize<'a>(
    text: &'a str,
) -> Result<(Vec<Token<'a>>, Vec<(usize, usize)>), (Error<'a>, usize)> {
    let mut tokens = Vec::new();
    let mut spans = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        let token = match c {
            ' ' | '\t' | '\n' => continue,
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
            '.' => Token::Dot,
            '[' => LeftBracket,
            ']' => RightBracket,
            '"' => {
                let end = closing_quote(&mut chars, '"').map_err(|error| (error, start))?;
                Token::Str(&text[start + 1..end])
            },
            '\'' => {
                let end = closing_quote(&mut chars, '\'').map_err(|error| (error, start))?;
                Token::Char(&text[start + 1..end])
            },
            '$' if chars.next_if(|&(_, d)| d == '"').is_some() => {
                let end = closing_quote(&mut chars, '"').map_err(|error| (error, start))?;
                Token::Interp(&text[start + 2..end])
            },
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
                    _ => return Err((Error::BadChar(c), start)),
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
                    _ => return Err((Error::BadChar(c), start)),
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
                        .ok_or((Error::Overflow, start))?;
                }
                if digits == 0 {
                    return Err((Error::Syntax("expected hex digits after '0x'"), start));
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
                        .ok_or((Error::Overflow, start))?;
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
                    "Char" => Token::Type(Type::Char),
                    "String" => Token::Type(Type::String),
                    "return" => Token::Return,
                    "Void" => Token::Void,
                    "for" => Token::For,
                    "break" => Token::Break,
                    "continue" => Token::Continue,
                    "do" => Token::Do,
                    "assert" => Token::Assert,
                    name => Token::Name(name),
                }
            }
            other => return Err((Error::BadChar(other), start))
        };
        tokens.push(token);
        spans.push((start, chars.peek().map_or(text.len(), |&(end, _)| end)));
    }
    Ok((tokens, spans))
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Op {
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
    pub(super) fn symbol(&self) -> &'static str {
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
