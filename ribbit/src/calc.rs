// Ribbit, FroggeOS's language. A line is split into tokens (token.rs),
// read into a tree (parser.rs, tree.rs), and then run (run.rs).

mod error;
mod parser;
mod run;
mod token;
mod tree;
mod value;

pub use error::{Error, Failure};
pub use value::{Type, Value, Variable, Variables};

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use parser::Parser;
use run::{Env, Flow, run};
use token::{Token, tokenize};
use tree::Block;

// A declared function.
pub struct Function<'a> {
    returns: Option<Type>,          // None for void
    params: Vec<(&'a str, Type)>,
    body: Block<'a>,
}

// All declared functions by name.
pub type Functions = BTreeMap<String, Function<'static>>;

// What the language requires from the machine its being run on.
pub trait Host {
    fn write(&mut self, text: &str);
    fn clear(&mut self);
    fn uptime_ms(&self) -> i64;
    fn beep(&mut self, hz: u32, ms: u64);
    fn sleep(&mut self, ms: u64);
    fn cancelled(&mut self) -> bool;
}

// The most stack a line may use while it reads and runs.
const MAX_STACK: usize = 48 * 1024;

// Roughly where the stack is now.
fn stack_address() -> usize {
    let marker = 0u8;
    &marker as *const u8 as usize
}

// Turns a line into its component statements.
fn parse<'a>(
    text: &'a str,
    vars: &Variables,
    stack_start: usize,
) -> Result<(Block<'a>, Vec<(&'a str, Function<'a>)>), Failure<'a>> {
    let (tokens, spans) = tokenize(text).map_err(|(error, at)| Failure { error, at: Some(at) })?;
    let mut parser = Parser::new(tokens, spans, stack_start);
    let program = parser.statements().map_err(|error| match parser.tokens[..] {
        // A line that doesn't parse, and starts with a name that isn't a
        // variable or being assigned, is most likely mistyped.
        [Token::Name(name), ..]
            if !matches!(parser.tokens.get(1), Some(Token::Equals | Token::LeftParen | Token::Dot)) && !vars.contains_key(name) =>
            {
                Failure { error: Error::UnknownName(name), at: Some(parser.spans[0].0) }
            }
        _ => Failure { error, at: Some(parser.position()) },
    })?;
    if parser.pos < parser.tokens.len() {
        return Err(Failure { error: Error::Syntax("unexpected '}'"), at: Some(parser.position()) });
    };
    Ok((program, parser.functions))
}

// Runs a line. Returns the value to print, if any.
pub fn evaluate<'a>(
    text: &'a str,
    vars: &mut Variables,
    functions: &mut Functions,
    host: &mut dyn Host,
) -> Result<Option<Value>, Failure<'a>> {
    let stack_start = stack_address();
    let (mut program, declared) = parse(text, vars, stack_start)?;
    if !declared.is_empty() {
        let text: &'static str = Box::leak(String::from(text).into_boxed_str());
        let (forever, declared) = parse(text, vars, stack_start)?;
        for (name, function) in declared {
            functions.insert(String::from(name), function);
        }
        program = forever;
    }
    let mut env = Env {
        globals: vars, functions, host, locals: Vec::new(), depth: 0,
        stack_start, statement: 0, calls: 0, expression: None
    };
    let flow = run(&program, &mut env).map_err(|error| Failure { error, at: Some(env.expression.unwrap_or(env.statement)) })?;
    match flow {
        Flow::Normal(value) | Flow::Return(value) => Ok(value),
        Flow::Break | Flow::Continue => Ok(None),
    }
}

pub fn is_complete(text: &str) -> bool {
    let Ok((tokens, _)) = tokenize(text) else {
        return true;
    };
    let open: i32 = tokens
        .iter()
        .map(|token| match token {
            Token::LeftBrace => 1,
            Token::RightBrace => -1,
            _ => 0,
        })
        .sum();
    open <= 0
}
