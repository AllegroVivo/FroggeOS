use lang::calc::{Variables, evaluate};

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
        ("1 ! 2", "Error: unexpected character '!'"),
        ("1 < = 2", "Error: expected a number, variable, or '('"),
    ]);
}

#[test]
fn variables() {
    check(&[
        ("x = 6 * 7", "42"),
        ("x + 1", "43"),
        ("y = x - 2", "40"),
        ("x = x + 1", "43"),
        ("(x + y) * 2", "166"),
        ("Big_1 = 10", "10"),
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
fn mistyped_commands() {
    check(&[
        ("hepl", "Error: 'hepl' is not a command or variable"),
        ("ecoh hi", "Error: 'ecoh' is not a command or variable"),
        ("foo =", "Error: unexpected end of input"),
        // Once x is a variable, `x y` is a syntax error, not a mistyped command.
        ("x = 5", "5"),
        ("x y", "Error: expected ';'"),
    ]);
}

#[test]
fn comparisons() {
    check(&[
        ("1 < 2", "1"),
        ("2 < 1", "0"),
        ("3 <= 3", "1"),
        ("5 >= 5", "1"),
        ("2 == 2", "1"),
        ("2 != 2", "0"),
        ("1 + 2 == 3", "1"), // + comes before ==
        ("(1 < 2) + (3 < 4)", "2"),
        ("3 > 2 > 1", "0"), // (3 > 2) is 1, and 1 > 1 is 0
        ("big = 7 > 3", "1"),
    ]);
}

#[test]
fn statements() {
    check(&[
        ("x = 1; y = 2; x + y", "3"),
        ("x = 1;", "1"),
        (";", ""),
        ("x = 1 y = 2", "Error: expected ';'"),
        ("}", "Error: unexpected '}'"),
    ]);
}

#[test]
fn if_else() {
    check(&[
        ("x = 5; if (x > 3) { y = 1 } else { y = 2 }", ""),
        ("y", "1"),
        ("if (x > 9) { y = 1 } else { y = 2 }; y", "2"),
        ("if (0) { z = 1 }; z", "Error: 'z' is not a command or variable"),
        ("if (-1) { z = 1 }; z", "1"), // any value but 0 counts as true
        ("n = 7; if (n < 5) { c = 1 } else if (n < 10) { c = 2 } else { c = 3 }; c", "2"),
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
        ("i = 0; s = 0; while (i < 10) { i = i + 1; s = s + i }; s", "55"),
        ("i = 0; while (i < 1000000) { i = i + 1 }; i", "1000000"), // just fits
        ("while (1) {}", "Error: loop stopped after 1000000 rounds"),
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
        ("a=0;b=0;while(a<3){a=a+1;if(a==2){b=b+10}else{b=b+1}};b", "12"),
        ("i=0;t=0;while(i<3){j=0;while(j<4){t=t+1;j=j+1};i=i+1};t", "12"),
        (&deep, "1"),
    ]);
}
