// Runs the tree.

use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use crate::notes;
use super::error::Error;
use super::token::Op;
use super::tree::{BuiltIn, Expr, Stmt};
use super::value::{Type, Value, Variable, Variables};
use super::{Functions, Host, MAX_STACK, stack_address};

// Variables seen by the currently executing code.
pub(super) struct Env<'a, 'v> {
    pub(super) globals: &'v mut Variables,
    pub(super) functions: &'v Functions,
    pub(super) host: &'v mut dyn Host,
    pub(super) locals: Vec<(&'a str, Variable)>,
    pub(super) depth: usize,
    pub(super) stack_start: usize,          // where the stack was when the line began
    pub(super) statement: usize,            // where the currently executing statement starts
    pub(super) calls: usize,                // how many function calls deep the line is
    pub(super) expression: Option<usize>    // the optional expression that errored
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

    fn note(&mut self, at: usize) {
        if self.calls == 0 {
            self.statement = at;
        }
    }
}

// Number `n` in hex, displayed in `width` digits.
#[inline(never)]
fn hex(n: i64, upper: bool, width: u8) -> Value {
    let w = width as usize;
    let text = if upper { format!("{:0w$X}", n, w = w) } else { format!("{:0w$x}", n, w = w) };
    Value::String(Rc::new(text))
}

// Works out the value of an expression. If an error arises, it also
// attempts to note the location of the failing expression if it hasn't
// already been noted.
fn eval<'a>(expr: &Expr<'a>, vars: &mut Env<'a, '_>) -> Result<Value, Error<'a>> {
    let value = eval_node(expr, vars);
    if value.is_err() && vars.calls == 0 {
        vars.expression = vars.expression.or(expr.at());
    }
    value
}

// The main functionality of `eval()`. Always inlined so the two share a stack frame.
#[inline(always)]
fn eval_node<'a>(expr: &Expr<'a>, vars: &mut Env<'a, '_>) -> Result<Value, Error<'a>> {
    vars.check_stack()?;
    match expr {
        Expr::Literal(value) => Ok(value.clone()),
        Expr::Variable(_, name) => {
            vars.get(*name).map(|variable| variable.value.clone()).ok_or(Error::UnknownName(name))
        },
        Expr::Negate(_, inner) => {
            let n = eval(inner, vars)?.int64("'-'")?;
            n.checked_neg().map(Value::Int64).ok_or(Error::Overflow)
        },
        Expr::Not(_, inner) => Ok(Value::Boolean(!eval(inner, vars)?.boolean("'!'")?)),
        Expr::BitNot(_, inner) => Ok(Value::Int64(!eval(inner, vars)?.int64("'~'")?)),
        Expr::And(_, left, right) => Ok(Value::Boolean(
            eval(left, vars)?.boolean("'&&'")? && eval(right, vars)?.boolean("'&&'")?,
        )),
        Expr::Or(_, left, right) => Ok(Value::Boolean(
            eval(left, vars)?.boolean("'||'")? || eval(right, vars)?.boolean("'||'")?,
        )),
        Expr::Binary(_, op, left, right) => {
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
                (
                    Op::Equal | Op::NotEqual | Op::Less | Op::LessEqual | Op::Greater | Op::GreaterEqual,
                    Value::Char(a),
                    Value::Char(b),
                ) => (a as i64, b as i64),
                (Op::Equal, Value::String(a), Value::String(b)) => return Ok(Value::Boolean(a == b)),
                (Op::NotEqual, Value::String(a), Value::String(b)) => return Ok(Value::Boolean(a != b)),
                (Op::Add, a @ Value::String(_), b) | (Op::Add, a, b @ Value::String(_)) => {
                    return Ok(Value::String(Rc::new(format!("{}{}", a, b))))
                },
                (Op::Equal | Op::NotEqual | Op::BitAnd | Op::BitOr | Op::BitXor, a, b) if a.type_of() != b.type_of() => {
                    return Err(Error::WrongType {place, wanted: a.type_name(), found: b.type_name() });
                },
                (_, a, b) => {
                    let found = if matches!(a, Value::Int64(_)) { b.type_name() } else { a.type_name() };
                    return Err(Error::WrongType {place, wanted: "Int64", found });
                },
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
        Expr::Call(_, name, args) => call(name, args, vars)?.ok_or(Error::NoValue(name)),
        Expr::BuiltIn(_, builtin, args) => {
            call_builtin(*builtin, args, vars)?.ok_or(Error::NoValue(builtin.name()))
        },
        Expr::Hex(_, inner, upper, width) => Ok(hex(eval(inner, vars)?.int64("hex")?, *upper, *width)),
        Expr::Index(_, text, index) => char_at(text, index, vars),
        Expr::Member(_, value, name) => member(value, name, vars),
    }
}

// The Char at a given string index (`s[i]`).
#[inline(never)]
fn char_at<'a>(text: &Expr<'a>, index: &Expr<'a>, vars: &mut Env<'a, '_>) -> Result<Value, Error<'a>> {
    let text = eval(text, vars)?;
    let index = eval(index, vars)?.int64("an index")?;
    let Value::String(s) = text else {
        return Err(Error::WrongType {place: "'[]'", wanted: "String", found: text.type_name() });
    };
    match usize::try_from(index).ok().and_then(|i| s.as_bytes().get(i)) {
        Some(&byte) => Ok(Value::Char(byte)),
        None => Err(Error::BadIndex { index, length: s.len() }),
    }
}

#[inline(never)]
fn member<'a>(value: &Expr<'a>, name: &'a str, vars: &mut Env<'a, '_>) -> Result<Value, Error<'a>> {
    match (eval(value, vars)?, name) {
        (Value::String(s), "Length") => Ok(Value::Int64(s.len() as i64)),
        (value, name) => Err(Error::NoMember { ty: value.type_name(), name }),
    }
}

pub(super) const MAX_LOOP_ROUNDS: u64 = 1_000_000;

pub(super) enum Flow {
    Normal(Option<Value>),  // statement reached the end
    Return(Option<Value>),  // statement was stopped by a `return`
    Break,                  // stopped by a `break` statement
    Continue,               // stopped by a `continue` statement
}

// Runs statements in order. Returns the value of the last item if that
// item is an expression or assignment.
pub(super) fn run<'a>(stmts: &[(usize, Stmt<'a>)], vars: &mut Env<'a, '_>) -> Result<Flow, Error<'a>> {
    let mut last = None;
    for (at, stmt) in stmts {
        vars.note(*at);
        last = match stmt {
            Stmt::Expr(Expr::Call(_, name, args)) => call(name, args, vars)?,
            Stmt::Expr(Expr::BuiltIn(_, builtin, args)) => call_builtin(*builtin, args, vars)?,
            Stmt::Expr(expr) => Some(eval(expr, vars)?),
            Stmt::Declare {name, ty, value, constant } => {
                let value = eval(value, vars)?;
                let ty = ty.unwrap_or(value.type_of());
                if value.type_of() != ty {
                    return Err(Error::VariableType { name, wanted: ty.name(), found: value.type_name() });
                }
                vars.declare(name, Variable { value: value.clone(), constant: *constant })?;
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
                variable.value = value.clone();
                Some(value)
            },
            Stmt::If(condition, then_block, else_block) => {
                let block = if eval(condition, vars)?.boolean("if")? { then_block } else { else_block };
                match run_block(block, vars)? {
                    Flow::Normal(_) => None,
                    flow => return Ok(flow),
                }
            },
            Stmt::Loop { keyword, init, condition, step, body, body_first } => {
                let start = vars.locals.len();
                vars.depth += 1;
                run(init, vars)?;
                vars.note(*at);
                let mut rounds = 0;
                while (*body_first && rounds == 0) || eval(condition, vars)?.boolean(keyword)? {
                    rounds += 1;
                    if rounds > MAX_LOOP_ROUNDS {
                        return Err(Error::LoopLimit);
                    }
                    if vars.host.cancelled() {
                        return Err(Error::Cancelled);
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
                    vars.note(*at);
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
            Stmt::Assert(condition, message) => {
                run_assert(condition, message, vars)?;
                None
            },
        }
    }
    Ok(Flow::Normal(last))
}

// Runs statements within a { } block.
fn run_block<'a>(stmts: &[(usize, Stmt<'a>)], vars: &mut Env<'a, '_>) -> Result<Flow, Error<'a>> {
    let start = vars.locals.len();
    vars.depth += 1;
    let flow = run(stmts, vars)?;
    vars.depth -= 1;
    vars.locals.truncate(start);
    Ok(flow)
}

// Stops the line if an assert's condition is False.
#[inline(never)]
fn run_assert<'a>(
    condition: &Expr<'a>,
    message: &Option<Expr<'a>>,
    vars: &mut Env<'a, '_>,
) -> Result<(), Error<'a>> {
    if eval(condition, vars)?.boolean("assert")? {
        return Ok(());
    }
    let message = match message {
        Some(message) => Some(eval(message, vars)?),
        None => None
    };
    Err(Error::AssertFailed(message))
}

// Runs a function built-in to the language.
fn call_builtin<'a>(
    builtin: BuiltIn,
    args: &[Expr<'a>],
    vars: &mut Env<'a, '_>,
) -> Result<Option<Value>, Error<'a>> {
    let mut values = Vec::new();
    for arg in args {
        values.push(eval(arg, vars)?);
    }
    match (builtin, values.as_slice()) {
        (BuiltIn::Write, [value]) => vars.host.write(&format!("{}", value)),
        (BuiltIn::WriteLine, []) => vars.host.write("\n"),
        (BuiltIn::WriteLine, [value]) => vars.host.write(&format!("{}\n", value)),
        (BuiltIn::Clear, []) => vars.host.clear(),
        (BuiltIn::Uptime, []) => return Ok(Some(Value::Int64(vars.host.uptime_ms()))),
        (BuiltIn::Sleep, [Value::Int64(ms @ 0..=10_000)]) => vars.host.sleep(*ms as u64),
        (BuiltIn::Beep, [Value::Int64(hz @ 20..=20_000), Value::Int64(ms @ 0..=10_000)]) => {
            vars.host.beep(*hz as u32, *ms as u64)
        },
        (BuiltIn::Sleep, [_]) => {
            return Err(Error::BadArgument { name: builtin.name(), needs: "0 - 10000 ms" });
        },
        (BuiltIn::Beep, [_, _]) => {
            return Err(Error::BadArgument { name: builtin.name(), needs: "20 - 20000 hz, 0 - 10000 ms" });
        },
        (BuiltIn::Play, [Value::String(tune)]) => play(tune, vars)?,
        (BuiltIn::Play, [_]) => {
            return Err(Error::BadArgument { name: builtin.name(), needs: "a String of valid notation" });
        },
        _ => {
            let wanted = match builtin {
                BuiltIn::Write | BuiltIn::WriteLine | BuiltIn::Sleep | BuiltIn::Play => 1,
                BuiltIn::Clear | BuiltIn::Uptime => 0,
                BuiltIn::Beep => 2,
            };
            return Err(Error::ArgumentCount { name: builtin.name(), wanted, found: values.len() });
        }
    }
    Ok(None)
}

// Plays a tune thru the host.
#[inline(never)]
fn play<'a>(tune: &str, vars: &mut Env<'a, '_>) -> Result<(), Error<'a>> {
    let Ok(tune) = notes::parse(tune) else {
        return Err(Error::BadArgument { name: "Speaker.Play", needs: "a valid String of Ribbit notation" });
    };
    for note in tune {
        if vars.host.cancelled() {
            return Err(Error::Cancelled);
        }
        match note.hz {
            Some(hz) => vars.host.beep(hz, note.ms),
            None => vars.host.sleep(note.ms),
        }
    }
    Ok(())
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
    if vars.host.cancelled() {
        return Err(Error::Cancelled);
    }
    let mut locals: Vec<(&'a str, Variable)> = Vec::new();
    for ((param, ty), arg) in function.params.iter().zip(args) {
        let value = eval(arg, vars)?;
        if value.type_of() != *ty {
            return Err(Error::VariableType { name: param, wanted: ty.name(), found: value.type_name() });
        }
        locals.push((param, Variable { value, constant: false }));
    }
    let caller = core::mem::replace(&mut vars.locals, locals);
    vars.calls += 1;
    let flow = run_block(&function.body, vars);
    vars.calls -= 1;
    vars.locals = caller;
    let value = match flow? {
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
