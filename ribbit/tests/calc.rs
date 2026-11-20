use ribbit::calc::{Functions, Host, Variables, evaluate, is_complete};

// Stands in for the kernel: keeps what Ribbit writes, and has a clock
// that always says 1234 ms.
struct TestHost {
    output: String,
}

impl Host for TestHost {
    fn write(&mut self, text: &str) {
        self.output.push_str(text);
    }

    fn clear(&mut self) {
        self.output.clear();
    }

    fn uptime_ms(&self) -> i64 {
        1234
    }
}

// Types the lines into one shell session and checks what the
// shell would print after each one: anything the line wrote,
// then its value or error.
fn check(session: &[(&str, &str)]) {
    let mut vars = Variables::new();
    let mut functions = Functions::new();
    for &(line, expected) in session {
        let mut host = TestHost { output: String::new() };
        let printed = match evaluate(line, &mut vars, &mut functions, &mut host) {
            Ok(Some(value)) => value.to_string(),
            Ok(None) => String::new(),
            Err(failure) => format!("Error: {}", failure.error),
        };
        assert_eq!(host.output + &printed, expected, "after typing: {}", line);
    }
}

#[test]
fn arithmetic() {
    check(&[
        ("6 * 7", "42"),
        ("1 + 2 * 3", "7"),
        ("(1 + 2) * 3", "9"),
        ("2 * (3 + 4) - 10 / 5", "12"),
        ("10 - 3 - 2", "5"), // left to right
        ("100 / 10 / 5", "2"),
        ("17 % 5 % 3", "2"),
        ("-7 / 2", "-3"), // rounds toward zero, like C
        ("- -5", "5"), // --5 is an error, as in C# (see shorthand)
        ("-(-(-(2 * 3)))", "-6"),
    ]);
}

#[test]
fn number_errors() {
    check(&[
        ("1 / 0", "Error: division by zero"),
        ("7 % 0", "Error: division by zero"),
        ("12345678901234567890", "Error: number too big"),
        ("9223372036854775807 + 1", "Error: number too big"),
        ("-9223372036854775807 - 1", "-9223372036854775808"), // the smallest i64
        ("-9223372036854775807 - 2", "Error: number too big"),
        ("-(-9223372036854775807 - 1)", "Error: number too big"),
    ]);
}

#[test]
fn syntax_errors() {
    check(&[
        ("(1 + 2", "Error: expected ')'"),
        ("1 +", "Error: unexpected end of input"),
        ("1 + )", "Error: expected a number, variable, or '('"),
        ("1 2", "Error: expected ';'"),
        ("2x", "Error: expected ';'"),
        ("3 $ 4", "Error: unexpected character '$'"),
        ("1 # 2", "Error: unexpected character '#'"),
        ("1 < = 2", "Error: expected a number, variable, or '('"),
    ]);
}

#[test]
fn variables() {
    check(&[
        ("Int64 x = 6 * 7", "42"),
        ("x + 1", "43"),
        ("Int64 y = x - 2", "40"),
        ("x = x + 1", "43"),
        ("(x + y) * 2", "166"),
        ("Int64 Big_1 = 10", "10"),
        ("big_1", "Error: 'big_1' is not a command or variable"), // case matters
        ("z + 1", "Error: 'z' is not a command or variable"),
        // A line that fails changes nothing.
        ("x =", "Error: unexpected end of input"),
        ("x = 1 2", "Error: expected ';'"),
        ("x = 1 / 0", "Error: division by zero"),
        ("x = = 5", "Error: expected a number, variable, or '('"),
        ("x", "43"),
    ]);
}

#[test]
fn declarations() {
    check(&[
        ("Int64 n = 5", "5"),
        ("Boolean done = n > 3", "True"),
        ("Int64 count", "0"), // everything starts zeroed
        ("Boolean flag", "False"),
        ("var total = n * 2", "10"), // var takes the type from the value
        ("var big = total > 5", "True"),
        ("const Int64 Max = 100", "100"),
        ("total = Max + 1", "101"),
        ("y = 5", "Error: 'y' isn't declared"),
        ("Max = 101", "Error: 'Max' is a constant"),
        ("Max", "100"),
        // A variable keeps its type, including one that var picked.
        ("n = true", "Error: 'n' needs Int64, not Boolean"),
        ("big = 1", "Error: 'big' needs Boolean, not Int64"),
        ("Int64 bad = false", "Error: 'bad' needs Int64, not Boolean"),
        ("bad", "Error: 'bad' is not a command or variable"), // it wasn't declared
        // At the prompt, declaring a name again replaces it.
        ("Boolean n = true", "True"),
        ("n = false", "False"),
    ]);
}

#[test]
fn declaration_errors() {
    check(&[
        ("var v", "Error: expected '='"),
        ("const Int64 C", "Error: expected '='"),
        ("const var c = 1", "Error: expected a type"),
        ("Int64 5 = 3", "Error: expected a name"),
        ("Int64 if = 3", "Error: expected a name"),
        ("Int64 = 5", "Error: expected a name"),
        ("Int64 x 5", "Error: expected ';'"),
    ]);
}

#[test]
fn block_scope() {
    check(&[
        // A block's variables end at its `}`.
        ("if (true) { Int64 t = 1 }; t", "Error: 't' is not a command or variable"),
        ("Int64 n = 0; while (n < 3) { Int64 sq = n * n; n = n + 1 }; n", "3"),
        ("sq", "Error: 'sq' is not a command or variable"),
        // Code in a block can see and change the prompt's variables.
        ("Int64 x = 1; if (true) { x = 5 }; x", "5"),
        // A block may hide a prompt variable until its `}`, like a C# local
        // hiding a field.
        ("Int64 seen = 0; if (true) { Int64 x = 9; seen = x }; seen", "9"),
        ("x", "5"),
        // An inner block can see an enclosing block's variables...
        ("Int64 r = 0; if (true) { Int64 a = 2; if (true) { r = a * 10 } }; r", "20"),
        // ...but, as in C#, can't reuse their names, and a block can't declare
        // the same name twice.
        ("if (true) { Int64 a = 1; if (true) { Int64 a = 2 } }", "Error: 'a' is already declared"),
        ("if (true) { Int64 a = 1; Int64 a = 2 }", "Error: 'a' is already declared"),
        // Sibling blocks can reuse a name, because the first one's is gone.
        ("if (true) { Int64 a = 1 } else { Int64 a = 2 }; if (true) { Int64 a = 3 }", ""),
        // Each round of a loop is a fresh block.
        ("Int64 s = 0; Int64 i = 0; while (i < 4) { Int64 d = i * 2; s = s + d; i = i + 1 }; s", "12"),
        ("if (true) { const Int64 K = 3; K = 4 }", "Error: 'K' is a constant"),
        // A block that fails leaves nothing behind.
        ("if (true) { Int64 q = 1; q = 1 / 0 }", "Error: division by zero"),
        ("q", "Error: 'q' is not a command or variable"),
    ]);
}

#[test]
fn mistyped_commands() {
    check(&[
        ("hepl", "Error: 'hepl' is not a command or variable"),
        ("ecoh hi", "Error: 'ecoh' is not a command or variable"),
        ("foo =", "Error: unexpected end of input"),
        // Once x is a variable, `x y` is a syntax error, not a mistyped command.
        ("Int64 x = 5", "5"),
        ("x y", "Error: expected ';'"),
    ]);
}

#[test]
fn comparisons() {
    check(&[
        ("1 < 2", "True"),
        ("2 < 1", "False"),
        ("3 <= 3", "True"),
        ("5 >= 5", "True"),
        ("2 == 2", "True"),
        ("2 != 2", "False"),
        ("1 + 2 == 3", "True"), // + comes before ==
        ("true == 1 < 2", "True"), // < comes before ==, as in C#
        ("Boolean big = 7 > 3", "True"),
        ("big == false", "False"),
        // A comparison gives a Boolean, and a Boolean isn't a number.
        ("(1 < 2) + (3 < 4)", "Error: '+' needs Int64, not Boolean"),
        ("3 > 2 > 1", "Error: '>' needs Int64, not Boolean"),
        ("1 == true", "Error: '==' needs Int64, not Boolean"),
        ("true == 1", "Error: '==' needs Boolean, not Int64"),
    ]);
}

#[test]
fn booleans() {
    check(&[
        ("true", "True"),
        ("!true", "False"),
        ("!!true", "True"),
        ("true && false", "False"),
        ("true || false", "True"),
        ("true || true && false", "True"), // && comes before ||
        ("1 < 2 && 2 < 3", "True"),
        ("!(1 > 2)", "True"),
        ("var t = true; var f = !t; t != f", "True"),
        ("!1", "Error: '!' needs Boolean, not Int64"),
        ("-true", "Error: '-' needs Int64, not Boolean"),
        ("true + 1", "Error: '+' needs Int64, not Boolean"),
        ("1 && true", "Error: '&&' needs Boolean, not Int64"),
    ]);
}

#[test]
fn short_circuit() {
    check(&[
        // && and || skip the right side when the left side decides the answer.
        ("false && 1 / 0 == 0", "False"),
        ("true || 1 / 0 == 0", "True"),
        ("true && 1 / 0 == 0", "Error: division by zero"),
        ("Int64 x = 0; x != 0 && 10 / x > 1", "False"),
        // Types are only checked while running (a checking pass is 6.7.3),
        // so a skipped right side isn't checked either.
        ("false && 5", "False"),
    ]);
}

#[test]
fn statements() {
    check(&[
        ("Int64 x = 1; Int64 y = 2; x + y", "3"),
        ("x = 1;", "1"),
        (";", ""),
        ("x = 1 y = 2", "Error: expected ';'"),
        ("}", "Error: unexpected '}'"),
    ]);
}

#[test]
fn if_else() {
    check(&[
        ("Int64 x = 5; Int64 y; if (x > 3) { y = 1 } else { y = 2 }", ""),
        ("y", "1"),
        ("if (x > 9) { y = 1 } else { y = 2 }; y", "2"),
        ("Int64 z; if (false) { z = 1 }; z", "0"),
        ("if (true) { z = 1 }; z", "1"),
        ("if (1) { z = 2 }", "Error: if needs Boolean, not Int64"), // write (x != 0)
        ("Int64 n = 7; Int64 c; if (n < 5) { c = 1 } else if (n < 10) { c = 2 } else { c = 3 }; c", "2"),
        ("if x > 1 { y = 1 }", "Error: expected '(' after if or while"),
        ("if (1 > 0 { 2 }", "Error: expected ')'"),
        ("if (1) y = 1", "Error: expected '{'"),
        ("if (1) { y = 1", "Error: expected '}'"),
        ("if = 5", "Error: expected '(' after if or while"),
        ("else", "Error: expected a number, variable, or '('"),
    ]);
}

#[test]
fn while_loops() {
    check(&[
        ("Int64 i = 0; Int64 s = 0; while (i < 10) { i = i + 1; s = s + i }; s", "55"),
        ("i = 0; while (i < 1000000) { i = i + 1 }; i", "1000000"), // just fits
        ("while (true) {}", "Error: loop stopped after 1000000 rounds"),
        ("while (1) {}", "Error: while needs Boolean, not Int64"),
        // Statements before an error stay done: i is still 0.
        ("i = 0; while (i < 3) { i = i + 1 / 0 }", "Error: division by zero"),
        ("i", "0"),
    ]);
}

#[test]
fn for_loops() {
    check(&[
        ("Int64 s = 0; for (Int64 i = 1; i <= 10; i = i + 1) { s = s + i }; s", "55"),
        // The loop's own variable is gone after the loop...
        ("i", "Error: 'i' is not a command or variable"),
        // ...and it can hide a prompt variable until the loop ends.
        ("Int64 i = 100; for (Int64 i = 0; i < 3; i = i + 1) {}; i", "100"),
        // A prompt variable can be the counter, and keeps its last value.
        ("Int64 n = 0; for (n = 1; n < 1000; n = n * 2) {}; n", "1024"),
        // Any part can be left out. With no condition it loops until a break.
        ("Int64 k = 0; for (; k < 3;) { k = k + 1 }; k", "3"),
        ("for (;;) {}", "Error: loop stopped after 1000000 rounds"),
        // Each round's body is a fresh block inside the loop's own one.
        ("Int64 t = 0; for (Int64 a = 1; a <= 3; a = a + 1) { Int64 sq = a * a; t = t + sq }; t", "14"),
        ("for (Int64 a = 0; a < 3; a = a + 1) { Int64 a = 5 }", "Error: 'a' is already declared"),
        (
            "t = 0; for (Int64 a = 1; a <= 3; a = a + 1) { for (Int64 b = 1; b <= 3; b = b + 1) { t = t + a * b } }; t",
            "36",
        ),
        ("for (; 1;) {}", "Error: for needs Boolean, not Int64"),
    ]);
}

#[test]
fn break_continue() {
    check(&[
        ("Int64 n = 0; while (true) { n = n + 1; if (n == 5) { break } }; n", "5"),
        ("for (;;) { n = n + 1; if (n >= 8) { break } }; n", "8"),
        // Nothing after a break runs.
        ("Int64 m = 0; while (true) { break; m = 99 }; m", "0"),
        // continue skips the rest of the round...
        (
            "Int64 odd = 0; Int64 i = 0; while (i < 10) { i = i + 1; if (i % 2 == 0) { continue } odd = odd + i }; odd",
            "25",
        ),
        // ...but a for's last part still runs, so the loop still counts.
        ("odd = 0; for (Int64 j = 1; j <= 10; j = j + 1) { if (j % 2 == 0) { continue } odd = odd + j }; odd", "25"),
        // break only leaves the innermost loop.
        (
            "Int64 c = 0; for (Int64 a = 0; a < 3; a = a + 1) { for (Int64 b = 0; b < 3; b = b + 1) { if (b == 1) { break } c = c + 1 } }; c",
            "3",
        ),
        // A return inside a loop leaves the loop and the function at once...
        ("Int64 FirstPower(Int64 over) { for (Int64 p = 1; ; p = p * 2) { if (p > over) { return p } } }", ""),
        ("FirstPower(100)", "128"),
        // ...and afterwards a declaration still goes to the prompt.
        ("FirstPower(5); Int64 after = 1", "1"),
        ("after", "1"),
    ]);
}

#[test]
fn loop_errors() {
    check(&[
        ("break", "Error: break and continue are only allowed within a loop body"),
        ("continue", "Error: break and continue are only allowed within a loop body"),
        ("if (true) { break }", "Error: break and continue are only allowed within a loop body"),
        ("Void Stop() { break }", "Error: break and continue are only allowed within a loop body"),
        // A for's last part isn't inside the loop's body.
        ("for (;; break) {}", "Error: break and continue are only allowed within a loop body"),
        ("for Int64 i = 0; i < 3; i = i + 1) {}", "Error: expected '(' after 'for'"),
        ("for (Int64 i = 0; i < 3) {}", "Error: expected ';'"),
        ("for (Int64 i = 0; i < 3; i = i + 1 {}", "Error: expected ')'"),
        ("for (;;) break", "Error: expected '{'"),
    ]);
}

#[test]
fn shorthand() {
    check(&[
        ("Int64 x = 5", "5"),
        ("x += 3", "8"),
        ("x -= 1", "7"),
        ("x *= 2 + 1", "21"), // the whole right side first: x = x * (2 + 1)
        ("x /= 4", "5"),
        ("x %= 3", "2"),
        // x++ prints the new value, like x += 1.
        ("x++", "3"),
        ("x--; x--", "1"),
        ("x", "1"),
        // The usual step of a for.
        ("Int64 s = 0; for (Int64 i = 0; i < 5; i++) { s += i }; s", "10"),
        ("for (Int64 i = 10; i > 0; i -= 3) { s++ }; s", "14"),
        // Like x = x + 1, they need a declared Int64 that can change.
        ("y++", "Error: 'y' is not a command or variable"),
        ("const Int64 C = 1; C += 1", "Error: 'C' is a constant"),
        ("Boolean b = true; b++", "Error: '+' needs Int64, not Boolean"),
        ("x += true", "Error: '+' needs Int64, not Boolean"),
        ("x /= 0", "Error: division by zero"),
        ("Int64 big = 9223372036854775807; big++", "Error: number too big"),
        // -- is always one token, as in C#: --5 isn't 5, but - -5 is.
        ("--5", "Error: '++' and '--' only work as a statement"),
        ("++x", "Error: '++' and '--' only work as a statement"),
        // Not inside an expression yet (LANGUAGE.md §6).
        ("Int64 y = x++", "Error: '++' and '--' only work as a statement"),
        ("x", "1"),
    ]);
}

#[test]
fn bits() {
    check(&[
        ("0x2A", "42"),
        ("0xff + 0XFF", "510"),
        ("0x7FFFFFFFFFFFFFFF", "9223372036854775807"),
        ("0x8000000000000000", "Error: number too big"),
        ("0x", "Error: expected hex digits after '0x'"),
        ("0xG", "Error: expected hex digits after '0x'"),
        ("12 & 10", "8"),
        ("12 | 10", "14"),
        ("12 ^ 10", "6"),
        ("~0", "-1"),
        ("~5", "-6"),
        ("1 << 4", "16"),
        ("-16 >> 2", "-4"), // keeps the sign
        // As in C#: no overflow check, and only the low 6 bits of the count.
        ("1 << 63", "-9223372036854775808"),
        ("1 << 64", "1"),
        // C#'s order: << comes after + and before the comparisons, then & ^ |.
        ("1 << 2 + 1", "8"),
        ("1 | 2 ^ 3 & 4", "3"), // 1 | (2 ^ (3 & 4))
        ("0xF0 & ~0x30 | 1 << 2", "196"),
        // C's trap: x & 1 == 0 means x & (1 == 0). Strict Booleans catch it.
        ("Int64 x = 6; x & 1 == 0", "Error: '&' needs Int64, not Boolean"),
        ("(x & 1) == 0", "True"),
        // On Booleans, & | ^ always use both sides: no short-circuit.
        ("true & false", "False"),
        ("true | false", "True"),
        ("true ^ true", "False"),
        ("false & 1 / 0 == 0", "Error: division by zero"),
        ("false && 1 / 0 == 0", "False"),
        ("true & 1", "Error: '&' needs Boolean, not Int64"),
        ("~true", "Error: '~' needs Int64, not Boolean"),
        // The shorthands.
        ("Int64 f = 0xF; f &= 6", "6"),
        ("f |= 0x10", "22"),
        ("f ^= 2", "20"),
        ("f <<= 2", "80"),
        ("f >>= 4", "5"),
        ("Boolean ok = true; ok &= false", "False"),
    ]);
}

#[test]
fn strings() {
    check(&[
        // A String prints as its text, without quotes.
        ("\"hi\"", "hi"),
        ("\"\"", ""),
        ("\"Hi, \" + \"there\"", "Hi, there"),
        ("String name = \"Ribbit\"", "Ribbit"),
        ("\"Hello, \" + name + \"!\"", "Hello, Ribbit!"),
        ("var s = \"a\"; s += \"b\"; s", "ab"),
        ("const String Greeting = \"hey\"", "hey"),
        // As in C#, + with a String turns the other side into text...
        ("\"x = \" + 6 * 7", "x = 42"),
        ("\"ok: \" + (1 < 2)", "ok: True"),
        // ...working left to right, like any other +.
        ("1 + 2 + \"!\"", "3!"),
        ("\"!\" + 1 + 2", "!12"),
        // Two Strings are equal when they hold the same text.
        ("name == \"Rib\" + \"bit\"", "True"),
        ("name != \"Ribbit\"", "False"),
        // The escapes: \" \\ \n \t \0
        ("\"a\\\"b\\\\c\"", "a\"b\\c"),
        ("\"line\\nnext\"", "line\nnext"),
        ("\"tab\\there\"", "tab\there"),
        ("\"nul\\0\"", "nul\0"),
        // Functions can take and give back Strings.
        ("String Twice(String t) { return t + t }", ""),
        ("Twice(\"ab\")", "abab"),
        // Errors.
        ("\"abc", "Error: missing a closing \""),
        ("\"a\\qb\"", "Error: unknown escape sequence"),
        ("String t", "Error: a String must be assigned a value"),
        ("Greeting += \"!\"", "Error: 'Greeting' is a constant"),
        ("name - 1", "Error: '-' needs Int64, not String"),
        ("-name", "Error: '-' needs Int64, not String"),
        ("\"a\" < \"b\"", "Error: '<' needs Int64, not String"),
        ("\"a\" & \"b\"", "Error: '&' needs Int64, not String"),
        ("name == 5", "Error: '==' needs String, not Int64"),
        ("Int64 n = \"5\"", "Error: 'n' needs Int64, not String"),
        ("if (name) {}", "Error: if needs Boolean, not String"),
    ]);
}

#[test]
fn console() {
    check(&[
        ("Console.WriteLine(\"Hello\")", "Hello\n"),
        ("Console.Write(\"a\"); Console.Write(1 + 1); Console.WriteLine()", "a2\n"),
        ("Console.WriteLine(1 < 2)", "True\n"),
        // Loops and functions can print as they run.
        ("for (Int64 i = 1; i <= 3; i++) { Console.WriteLine(\"i = \" + i) }", "i = 1\ni = 2\ni = 3\n"),
        ("Void Greet(String who) { Console.WriteLine(\"Hi, \" + who) }; Greet(\"Ribbit\")", "Hi, Ribbit\n"),
        // A line's own value comes after what it wrote.
        ("Console.Write(\"x is \"); 42", "x is 42"),
        ("Console.Write(\"gone\"); Console.Clear(); Console.Write(\"fresh\")", "fresh"),
        ("Timer.Uptime()", "1234"), // the test host's clock
        ("Timer.Uptime() + 1", "1235"),
        // Errors.
        ("Console.ReadLine()", "Error: 'Console.ReadLine' is not a built-in function"),
        ("Console.Write()", "Error: 'Console.Write' takes 1 argument, not 0"),
        ("Console.WriteLine(1, 2)", "Error: 'Console.WriteLine' takes 1 argument, not 2"),
        ("Console.Clear(1)", "Error: 'Console.Clear' takes 0 arguments, not 1"),
        ("Console.WriteLine", "Error: expected '('"),
        ("Console.", "Error: expected a name after '.'"),
        ("Console.WriteLine(1 / 0)", "Error: division by zero"),
        // As with a Void function of your own, it runs before the error.
        ("Int64 n = Console.WriteLine(1)", "1\nError: 'Console.WriteLine' is void and has no value to be used"),
    ]);
}

#[test]
fn interpolation() {
    check(&[
        ("Int64 x = 42", "42"),
        ("$\"x = {x}\"", "x = 42"),
        // Always a String, even with nothing but a hole...
        ("$\"{x}\"", "42"),
        ("$\"{x}\" + 1", "421"),
        // ...and each hole is a whole expression.
        ("$\"{x / 2 + 1} and {x == 42}\"", "22 and True"),
        ("String name = \"Ribbit\"; $\"Hi, {name}!\"", "Hi, Ribbit!"),
        // Hex: X or x, and a width.
        ("$\"hex {x:X}, {255:x}, {x:X8}\"", "hex 2A, ff, 0000002A"),
        ("$\"{-1:X}\"", "FFFFFFFFFFFFFFFF"),
        // {{ and }} are braces, and the usual escapes work.
        ("$\"{{x}} is {x}\"", "{x} is 42"),
        ("$\"tab\\t{x}\"", "tab\t42"),
        ("$\"\"", ""),
        ("for (Int64 i = 1; i <= 2; i++) { Console.WriteLine($\"i = {i}\") }", "i = 1\ni = 2\n"),
        ("String Show(Int64 n) { return $\"<{n}>\" }; Show(7)", "<7>"),
        // Errors.
        ("$\"{x\"", "Error: missing closing '}' for string interpolation. escape brackets with '{{'"),
        ("$\"x}\"", "Error: missing opening '{' for string interpolation. escape brackets with '}}'"),
        ("$\"{x y}\"", "Error: an interpolate {hole} may only hold one expression"),
        ("$\"{}\"", "Error: unexpected end of input"),
        ("$\"{x:D}\"", "Error: an interpolation format must be in the style of {n:x8}"),
        ("$\"{x:X999}\"", "Error: an interpolation format must be in the style of {n:x8}"),
        ("$\"{name:X}\"", "Error: hex needs Int64, not String"),
        ("$\"{y}\"", "Error: 'y' is not a command or variable"),
        ("$\"abc", "Error: missing a closing \""),
    ]);
}

#[test]
fn multi_line() {
    // The shell keeps reading lines while a { is still open...
    assert!(!is_complete("Int64 Sq(Int64 n) {"));
    assert!(!is_complete("for (Int64 i = 0; i < 3; i++) {\n    if (i == 1) {\n    }"));
    assert!(!is_complete("{"));
    // ...and runs them once every { has its }.
    assert!(is_complete("Int64 Sq(Int64 n) {\n    return n * n\n}"));
    assert!(is_complete("{ }"));
    assert!(is_complete("6 * 7"));
    assert!(is_complete(""));
    // Braces inside strings don't count.
    assert!(is_complete("\"{\""));
    assert!(is_complete("$\"{{ and {1}\""));
    // A mistake counts as ready, so its error shows at once.
    assert!(is_complete("}"));
    assert!(is_complete("\"{"));
    assert!(is_complete("Int64 F() { ` }"));
    // The shell joins the lines with newlines, which only separate tokens.
    check(&[
        ("Int64 Sq(Int64 n) {\n    return n * n\n}", ""),
        ("Sq(7)", "49"),
        ("for (Int64 i = 1; i <= 2; i++) {\n    Console.WriteLine($\"{i}: {Sq(i)}\");\n}", "1: 1\n2: 4\n"),
        ("Int64 a = 1;\nInt64 b = 2;\na + b", "3"),
        ("1 +\t2", "3"),
        // A new line doesn't end a statement, so ; still goes between them.
        ("Int64 c = 1\nc", "Error: expected ';'"),
        // A blank line runs an unfinished block, which says what's missing.
        ("if (true) {", "Error: expected '}'"),
        ("Int64 F() {\n    return 1", "Error: expected '}'"),
    ]);
}

#[test]
fn nesting() {
    // A whole 78-character shell line of brackets, 38 deep, fits easily.
    let deep = format!("{}1{}", "(".repeat(38), ")".repeat(38));
    check(&[
        ("var a=0;var b=0;while(a<3){a=a+1;if(a==2){b=b+10}else{b=b+1}};b", "12"),
        ("var i=0;var t=0;while(i<3){var j=0;while(j<4){t=t+1;j=j+1};i=i+1};t", "12"),
        (&deep, "1"),
    ]);
}

// `inside`, in `depth` pairs of brackets.
fn bracketed(depth: usize, inside: &str) -> String {
    format!("{}{}{}", "(".repeat(depth), inside, ")".repeat(depth))
}

#[test]
fn deep_nesting() {
    // Reading a line that nests too deeply stops with an error instead of
    // overflowing the stack, whatever does the nesting.
    let too_deep = "Error: nested too deeply (over 48 KB of stack)";
    let in_hole = |depth| format!("$\"{{{}}}\"", bracketed(depth, "1"));
    check(&[
        (&bracketed(200, "1"), too_deep),
        (&format!("{}true", "!".repeat(1000)), too_deep),
        (&format!("{}{}", "if (true) {\n".repeat(100), "}\n".repeat(100)), too_deep),
        (&format!("{}{{ }}", "if (false) { } else ".repeat(300)), too_deep),
        (&in_hole(200), too_deep),
        // A {hole} shares its line's budget: 35 brackets around one and 35
        // in it are too many together, though either half alone would fit.
        (&bracketed(20, &in_hole(20)), "1"),
        (&bracketed(35, &in_hole(35)), too_deep),
    ]);
}

#[test]
fn functions() {
    check(&[
        ("Int64 Square(Int64 n) { return n * n }", ""),
        ("Square(7)", "49"),
        ("Square(2) + Square(3)", "13"),
        ("Square(Square(3))", "81"),
        ("Boolean IsEven(Int64 n) { return n % 2 == 0 }", ""),
        ("IsEven(10) && !IsEven(7)", "True"),
        ("Int64 Add(Int64 a, Int64 b) { return a + b }; Add(2, 40)", "42"),
        // A body can have statements, loops, and a `return` from inside a loop.
        ("Int64 Sum(Int64 n) { Int64 s = 0; while (n > 0) { s = s + n; n = n - 1 } return s }", ""),
        ("Sum(10)", "55"),
        ("Int64 Root(Int64 x) { Int64 i = 0; while (true) { i = i + 1; if (i * i > x) { return i - 1 } } }", ""),
        ("Root(50)", "7"),
        // A Void function gives nothing back, but can change the prompt's variables.
        ("Int64 count = 0", "0"),
        ("Void Bump() { count = count + 1 }", ""),
        ("Bump(); Bump(); count", "2"),
        ("Void Early(Int64 n) { if (n > 0) { return } count = 100 }; Early(1); count", "2"),
        // Parameters belong to the function, so the caller's `n` stays put.
        ("Int64 n = 5; Square(3); n", "5"),
        // Declaring a function again replaces it.
        ("Int64 Square(Int64 n) { return n * n * n }; Square(2)", "8"),
    ]);
}

#[test]
fn recursion() {
    check(&[
        ("Int64 Fib(Int64 n) { if (n < 2) { return n } return Fib(n - 1) + Fib(n - 2) }", ""),
        ("Fib(10)", "55"),
        ("Fib(20)", "6765"),
        ("Int64 Fact(Int64 n) { if (n <= 1) { return 1 } return n * Fact(n - 1) }", ""),
        ("Fact(20)", "2432902008176640000"),
        ("Fact(21)", "Error: number too big"),
        // Runaway recursion stops with an error instead of crashing.
        ("Int64 Forever(Int64 n) { return Forever(n + 1) }", ""),
        ("Forever(0)", "Error: nested too deeply (over 48 KB of stack)"),
        ("Void Spin() { Spin() }", ""),
        ("Spin()", "Error: nested too deeply (over 48 KB of stack)"),
        // Everything still works afterwards.
        ("Fib(12)", "144"),
    ]);
}

#[test]
fn function_errors() {
    check(&[
        ("Nope(1)", "Error: 'Nope' is not a function"),
        ("Int64 Half(Int64 n) { return n / 2 }", ""),
        ("Half()", "Error: 'Half' takes 1 argument, not 0"),
        ("Half(1, 2)", "Error: 'Half' takes 1 argument, not 2"),
        ("Half(true)", "Error: 'n' needs Int64, not Boolean"),
        ("Half(4", "Error: expected ',' or ')'"),
        ("Int64 Bad(Int64 n) { return n > 0 }", ""),
        ("Bad(1)", "Error: 'Bad' needs Int64, not Boolean"),
        ("Int64 NoEnd(Int64 n) { n = n + 1 }", ""),
        ("NoEnd(1)", "Error: 'NoEnd' must return a value"),
        ("Void Quiet() { return }", ""),
        ("Quiet() + 1", "Error: 'Quiet' is void and has no value to be used"),
        ("Int64 v = Quiet()", "Error: 'Quiet' is void and has no value to be used"),
        ("Void Loud() { return 5 }", ""),
        ("Loud()", "Error: 'Loud' needs Void, not Int64"),
        ("return 5", "Error: return is only allowed within a function body"),
        ("Void x = 5", "Error: cannot assign void to a variable"),
        ("if (true) { Int64 Inner() { return 1 } }", "Error: functions may only be declared at the prompt"),
        ("Int64 Two(Int64 a, Int64 a) { return a }", "Error: 'a' is already declared"),
        ("Int64 Pair(Int64 a Int64 b) { return a }", "Error: expected ',' or ')'"),
        ("Int64 NoType(a) { return a }", "Error: expected a parameter type"),
        // A function can't see the caller's block variables.
        ("Int64 Peek() { return hidden }", ""),
        ("if (true) { Int64 hidden = 1; Peek() }", "Error: 'hidden' is not a command or variable"),
    ]);
}

// The text before the spot that its mistake points to. None means the
// mistake has no place, because it only showed up while running.
fn before_mistake(text: &str) -> Option<&str> {
    let mut host = TestHost { output: String::new() };
    let failure = evaluate(text, &mut Variables::new(), &mut Functions::new(), &mut host).err()?;
    failure.at.map(|at| &text[..at])
}

#[test]
fn error_positions() {
    // A missing token is pointed at just after the token before it,
    // where it belongs...
    assert_eq!(before_mistake("6 * (7 + 1"), Some("6 * (7 + 1"));
    assert_eq!(before_mistake("Sq(7 8)"), Some("Sq(7"));
    assert_eq!(before_mistake("if x > 1 { }"), Some("if"));
    assert_eq!(before_mistake("Int64 a = 1 a"), Some("Int64 a = 1"));
    assert_eq!(before_mistake("if (true) {"), Some("if (true) {"));
    assert_eq!(
        before_mistake("if (true) {\n    Int64 a = 1\n    a\n}"),
        Some("if (true) {\n    Int64 a = 1")
    );
    // ...and any other mistake at the token that's wrong.
    assert_eq!(before_mistake("1 + )"), Some("1 + "));
    assert_eq!(before_mistake("6 * 7 }"), Some("6 * 7 "));
    assert_eq!(before_mistake("1 + x++"), Some("1 + x"));
    assert_eq!(before_mistake("Console.Writeline(1)"), Some("Console."));
    assert_eq!(before_mistake("if (true) {\n    return 1\n}"), Some("if (true) {\n    "));
    assert_eq!(before_mistake("ech hello"), Some(""));
    // A mistake inside a token points at the token's start...
    assert_eq!(before_mistake("3 $ 4"), Some("3 "));
    assert_eq!(before_mistake("x = \"abc"), Some("x = "));
    assert_eq!(before_mistake("x = 0xZ"), Some("x = "));
    assert_eq!(before_mistake("x = \"\\q\""), Some("x = "));
    // ...which for $"..." is the start of the whole string.
    assert_eq!(before_mistake("x = $\"{1 +}\""), Some("x = "));
    // Mistakes found while running have no place.
    assert_eq!(before_mistake("1 / 0"), None);
    assert_eq!(before_mistake("y = 5"), None);
}
