// The tree that a line is read into: expressions and statements.

use alloc::boxed::Box;
use alloc::vec::Vec;
use super::token::Op;
use super::value::{Type, Value};

pub(super) enum Expr<'a> {
    Literal(Value),
    Variable(usize, &'a str),                           // x
    Negate(usize, Box<Expr<'a>>),                       // !
    Not(usize, Box<Expr<'a>>),                          //
    BitNot(usize, Box<Expr<'a>>),                       //
    Binary(usize, Op, Box<Expr<'a>>, Box<Expr<'a>>),    // <</>>
    And(usize, Box<Expr<'a>>, Box<Expr<'a>>),           // &&
    Or(usize, Box<Expr<'a>>, Box<Expr<'a>>),            // ||
    Call(usize, &'a str, Vec<Expr<'a>>),                // F()
    BuiltIn(usize, BuiltIn, Vec<Expr<'a>>),             // e.g. Console.WriteLine(expr)
    Hex(usize, Box<Expr<'a>>, bool, u8),                // 0xXX
    Index(usize, Box<Expr<'a>>, Box<Expr<'a>>),         // x[i]
    Member(usize, Box<Expr<'a>>, &'a str),              // x.Length
}

impl Expr<'_> {
    pub(super) fn at(&self) -> Option<usize> {
        match self {
            Expr::Literal(_) => None,
            Expr::Variable(at, ..) | Expr::Negate(at, ..) | Expr::Not(at, ..) | Expr::BitNot(at, ..)
            | Expr::Binary(at, ..) | Expr::And(at, ..) | Expr::Or(at, ..) | Expr::Call(at, ..)
            | Expr::BuiltIn(at, ..) | Expr::Hex(at, ..) | Expr::Index(at, ..) | Expr::Member(at, ..) => Some(*at),
        }
    }
}

#[derive(Copy, Clone)]
pub(super) enum BuiltIn {
    Write,              // Console.Write(str)
    WriteLine,          // Console.WriteLine(str) / Console.WriteLine()
    Clear,              // Console.Clear()
    Uptime,             // Timer.Uptime()
    Sleep,              // Timer.Sleep(ms)
    Beep,               // Speaker.Beep(hz, ms)
    Play,               // Speaker.Play(tune)
}

impl BuiltIn {
    pub(super) fn find(namespace: &str, name: &str) -> Option<BuiltIn> {
        Some(match (namespace, name) {
            ("Console", "Write") => BuiltIn::Write,
            ("Console", "WriteLine") => BuiltIn::WriteLine,
            ("Console", "Clear") => BuiltIn::Clear,
            ("Timer", "Uptime") => BuiltIn::Uptime,
            ("Timer", "Sleep") => BuiltIn::Sleep,
            ("Speaker", "Beep") => BuiltIn::Beep,
            ("Speaker", "Play") => BuiltIn::Play,
            _ => return None,
        })
    }

    pub(super) fn name(self) -> &'static str {
        match self {
            BuiltIn::Write => "Console.Write",
            BuiltIn::WriteLine => "Console.WriteLine",
            BuiltIn::Clear => "Console.Clear",
            BuiltIn::Uptime => "Timer.Uptime",
            BuiltIn::Sleep => "Timer.Sleep",
            BuiltIn::Beep => "Speaker.Beep",
            BuiltIn::Play => "Speaker.Play",
        }
    }
}

pub(super) enum Stmt<'a> {
    Expr(Expr<'a>),
    Assign(&'a str, Expr<'a>),
    Declare { name: &'a str, ty: Option<Type>, value: Expr<'a>, constant: bool },
    If(Expr<'a>, Block<'a>, Block<'a>), // condition, then, else
    Return(Option<Expr<'a>>),
    Break,
    Continue,
    Loop {
        keyword: &'static str,
        init: Block<'a>,
        condition: Expr<'a>,
        step: Block<'a>,
        body: Block<'a>,
        body_first: bool        // as in a `do` loop
    },
    Assert(Expr<'a>, Option<Expr<'a>>), // condition, message
}

pub(super) type Block<'a> = Vec<(usize, Stmt<'a>)>;
