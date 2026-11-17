use ribbit::calc::{Variables, evaluate};

// Types the lines into one shell session and checks what the
// shell would print after each one.
fn check(session: &[(&str, &str)]) {
    let mut vars = Variables::new();
    for &(line, expected) in session {
        let printed = match evaluate(line, &mut vars) {
            Ok(Some(value)) => value.to_string(),
            Ok(None) => String::new(),
            Err(error) => format!("Error: {}", error),
        };
        assert_eq!(printed, expected, "after typing: {}", line);
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
        ("--5", "5"),
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
        ("1 & 2", "Error: unexpected character '&'"),
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
        // Declaring a name again replaces it, until blocks get their own scope.
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
fn nesting() {
    // 38 brackets deep is the most that fits in a 78-character shell line.
    let deep = format!("{}1{}", "(".repeat(38), ")".repeat(38));
    check(&[
        ("var a=0;var b=0;while(a<3){a=a+1;if(a==2){b=b+10}else{b=b+1}};b", "12"),
        ("var i=0;var t=0;while(i<3){var j=0;while(j<4){t=t+1;j=j+1};i=i+1};t", "12"),
        (&deep, "1"),
    ]);
}
