# Ribbit: the FroggeOS language

**Status (2026-11-22):** design decided (roadmap ⚑ 6.1). Ribbit's core now runs at the FroggeOS prompt: Int64, Boolean, Char and String values, blocks, loops, functions and recursion. ROADMAP.md tracks what's built. Ribbit scripts are `.rbt` files, and the Rust crate is `ribbit`.

**In one line:** C#-style syntax with CLR type names, C#'s keywords and operator precedence, full generics, `$"..."` strings, .NET-style namespaces, reference counting instead of a garbage collector, raw pointers inside `unsafe`, and argparse-style shell commands next to the code.

**On size:** this is most of C#, plus pointers. A C# compiler is a large program, so each feature has its own roadmap step, and milestone M2 only needs the small core (§11). The costliest features are generics, `async`/`await`, `yield`, and exceptions in the x86 compiler (6.8).

---

## 1. A taste

```csharp
using Speaker;

namespace Demo
{
    Int64 Fib(Int64 n)
    {
        if (n < 2) {
            return n;
        }
        return Fib(n - 1) + Fib(n - 2);
    }

    IEnumerable<Int64> Evens(Int64 max)
    {
        for (Int64 i = 0; i <= max; i += 2) {
            yield return i;
        }
    }
}

foreach (Int64 n in Demo.Evens(10)) {
    Console.WriteLine($"Fib({n}) = {Demo.Fib(n)}");
}
Beep(440, 200);
```

At the prompt:

```
> uptime
12.4 s
> beep 440 --ms 200
> Int64 x = 6 * 7;
42
> x + 1;
43
> y = 5;
Error: 'y' isn't declared
> Console.WriteLine($"x is {x:X}");
x is 2A
```

## 2. The prompt: commands and code

- **The first word decides.** If it's a registered command, the line is a command: plain words plus `-f`/`--flag` options. They're checked against the arguments the command declares, like Python's argparse, and there's no `;`. `help beep` and `beep --help` print usage text generated from those declarations (step 6.6.1). Until then, the shell's current commands keep working as they do now.
- **Anything else is code,** and it needs its `;`. Declarations, `using`, functions and classes typed at the prompt last for the whole session. If the last statement is an expression, its value is printed.
- **A block can span lines.** While a `{` is still open, the prompt shows `. ` and keeps reading. A new line only separates tokens, so `;` still goes between statements. To give up on a block, press Enter on a blank `. ` line: it runs what's there, which reports the missing `}`.
- **A mistake is pointed at.** A `^` goes under the spot, either on the line just typed or on a copy of an earlier line of the block, whose number the message gives. A missing token is pointed at just after the token before it, as in C#. A mistake found while running points at the operator, name, `[` or call that went wrong, or at the start of its statement when the whole statement is at fault (such as assigning to a `const`). One inside a function points at the call on the line that was typed. If the line printed something first, the line is shown again above the `^`.
- **Commands are lowercase.** You can't declare a variable or function with a command's name.
- **Code can't use a command's output.** Code calls the matching function instead (`Timer.Uptime()`). A command and its function can share one Rust implementation.

A command is declared in Rust, roughly like this. It's a design sketch, not final code:

```rust
Command {
    name: "beep",
    about: "Play a tone on the PC speaker.",
    args: &[
        Arg::positional("hz", Kind::Int64, "frequency in Hz"),
        Arg::option("ms", Kind::Int64, "how long, in milliseconds").default(100),
    ],
    run: beep,
}
```

## 3. Keywords

All keywords are lowercase and fully reserved. C# treats `var`, `yield`, `async`, `await` and `where` as contextual (usable as names outside their special place). Here they're reserved everywhere, which keeps the tokenizer simple.

| Group | Keywords |
|---|---|
| Control flow | `if else while do for break continue return switch case default` |
| Sequences | `foreach in yield` |
| Errors | `try catch finally throw` |
| Cleanup and checks | `using lock assert` |
| Values | `true false null this base` |
| Declarations | `var const static readonly volatile extern params ref out` |
| Unsafe code | `unsafe fixed sizeof` |
| Kinds of type | `struct class interface enum delegate namespace new` |
| Access and inheritance | `public private protected virtual override abstract sealed` |
| Type operators | `is as typeof operator` |
| Generics | `where` |
| Async | `async await` |

`using` has two jobs, as in C#. `using Speaker;` imports a namespace, and `using (...) { }` cleans up at the end of a block.

**Reserved type names:** `Void Boolean Char Byte SByte Int16 UInt16 Int32 UInt32 Int64 UInt64 Single Double String Object`. There are no C# aliases (`int`, `long`, `string`): each type has one name.

**Left out on purpose:**
- C#: `goto`, `checked`/`unchecked`, properties (`get`/`set`), `event` and LINQ.
- Python: the word spellings (`and`/`or`/`not`, `elif`, `pass`, `del`, `in` as a test) and `import`/`from`.

`using` already covers Python's `with`, and `=>` covers `lambda`.

## 4. Types

| Kind | Types | Size |
|---|---|---|
| Values | `Boolean`, `Char`, `Byte`, `SByte` | 1 byte |
| | `Int16`, `UInt16` | 2 |
| | `Int32`, `UInt32` | 4 |
| | `Int64`, `UInt64` | 8 |
| | `Single`, `Double` (software floats until 2.2.4) | 4 / 8 |
| | `struct`s, `enum`s, pointers `T*` | varies |
| References | `String`, `Object`, arrays `T[]`, classes, interfaces, delegates | 8 (the address) |
| Return type only | `Void` | none |

- **`Boolean` is strict, as in C#.** Comparisons give `true` or `false`, and `if`, `while`, `!`, `&&` and `||` only accept a Boolean. `if (n)` has to be written `if (n != 0)`. It prints as `True` or `False`, like C#'s `ToString()`.
- **A `Char` is 1 byte,** a code-page-437 character: what the VGA text screen shows and what the keyboard driver produces. C#'s Char is 2 bytes (UTF-16). `Char` and `Byte` are the same size but different types: a `Byte` prints as a number (`70`), a `Char` as text (`F`). For now Chars, and the characters in a String, are ASCII only (`'\x00'` to `'\x7F'`); the rest of code page 437 comes when strings are stored as bytes.
- **`String` works like C#'s.** It's immutable and supports `+`, `.Length` and `s[i]` (which gives a `Char`). At the prompt it prints as its text, without quotes, as `Console.WriteLine` would. Until Ribbit has `null`, a String must be given a value: `String s;` is an error.
- **`Object` is the base of every class.** `is`, `as` and `typeof` need it.
- **Conversions follow C#.** Widening is automatic (`Int32` to `Int64`). Narrowing needs a cast: `(Byte)x`. Integer literals adapt to the type they're used with.
- **Overflow and division by zero are always errors.** C# wraps silently on overflow by default, so this is one place we differ. Shifts are the exception, as in C#: bits shifted out are lost and only the low 6 bits of the count are used, so `1 << 63` is the most negative Int64 and `1 << 64` is 1.
- **Everything starts zeroed:** `0`, `false`, `'\0'` or `null`. C# instead refuses to compile a read of an unassigned local; zeroing is simpler.
- **References can be `null`.** Using a null reference is an error.

## 5. Tokens

- **Names:** letters, digits and `_`, not starting with a digit. Case matters. Follow the .NET conventions: PascalCase for types, functions and namespaces, camelCase for locals and parameters, and lowercase for shell commands.
- **Numbers:** `42`, `0x2A` and `3.14`
- **Characters:** `'A'`, `'\n'`, `'\x41'`
- **Strings:** `"..."`, with the escapes `\n \t \\ \" \' \0 \xNN`
- **Interpolated strings:** `$"x = {x}, hex {x:X8}"`. Each `{...}` is ordinary code and is checked like any other expression. As in C#, `{{` and `}}` stand for `{` and `}`. After a `:`, `X` or `x` prints hex in upper or lower case, padded with zeros to the width that follows: `{255:X4}` is `00FF`. For now, unlike C#, a `{...}` can't hold a `"`.
- **Comments:** `//` to the end of the line, and `/* ... */`
- **Whitespace:** spaces, tabs and newlines only separate tokens.

## 6. Statements

- **Declarations:** `Int64 x = 5;`, `var w = new Window();` (`var` takes the type from the right-hand side) and `const Int64 Max = 10;`
  - At the prompt, declaring a name again replaces it, as in C#'s interactive window.
  - Inside a block, as in C#, a name can't be declared twice or reuse the name of an enclosing block's variable. A block may hide a variable from the prompt until its `}`, the way a C# local can hide a field.
- **Assignment:** `x = e;`, `x += e;` (and `-= *= /= %= &= |= ^= <<= >>=`), `x++;` and `x--;`
  - These are statements only in version 1. C# also allows them inside expressions (`a[i++]`), and adding that later won't break existing code.
  - At the prompt, `x++` prints x's new value, as `x += 1` does. (C#'s prompt prints the old one, because there `x++` is an expression.)
  - As in C#, `++` and `--` are always one token, so `--5` is an error. Write `- -5` to negate twice.
- **Branches and loops:** `if (c) { } else if (c) { } else { }`, `while (c) { }`, `do { } while (c);`, `for (init; cond; step) { }` and `foreach (T x in seq) { }`
- **Switch:** `switch (x) { case 1: ... break; default: ... break; }`. As in C#, one case can't fall through into the next.
- **Jumps:** `break;`, `continue;`, `return e;` and `return;`
- **Errors:** `try { } catch (Exception e) { } finally { }` and `throw new Exception("...");`
- **Cleanup:** `using (var f = File.Open("/a")) { }` calls `f.Dispose()` at the end of the block, even after an error.
- **Locking:** `lock (m) { }` holds a lock for the whole block.
- **Checks:** `assert x > 0, "x must be positive";` uses Python's form, and the message is optional. A failed check stops the line with `assertion failed`, followed by its message if it has one. As in Python, the message is only worked out when the check fails.
- **Braces:** every body needs `{ }`, unlike C#. This avoids the "dangling else" ambiguity.

**The `;` rule:** every statement ends with `;`, at the prompt too. The exception is the `}` that closes a block, function, class, struct, enum, interface or namespace, which needs no `;` after it. `do { } while (c);` keeps its `;`.

## 7. Operators

From tightest to loosest, in C#'s order:

```
x.y  f(x)  a[i]  p->y  new  typeof  sizeof
-x  !x  ~x  (T)x  await  &x  *x
*  /  %
+  -
<<  >>
<  >  <=  >=  is  as
==  !=
&
^
|
&&
||
c ? a : b
=  +=  -=  *=  /=  %=  &=  |=  ^=  <<=  >>=  =>
```

C's precedence has a trap: `x & 1 == 0` means `x & (1 == 0)`. Strict Booleans turn that into a type error, so C#'s order is safe to use. ☆ C#'s null shortcuts `??` and `?.` can come later.

## 8. Functions, delegates, lambdas and yield

- **Where functions live:** `Int64 Fib(Int64 n) { }` can sit at the top level or inside a namespace. Unlike C#, a function doesn't need a class around it.
- **Calls** always need `()`.
- **Parameters:** `params Int64[] values`, `ref Int64 x` and `out Int64 result`
- **Built-ins:** `extern Void Beep(Int64 hz, Int64 ms);` declares a function whose body is written in Rust. The built-in library becomes a FroggeOS source file full of these.
- **Recursion limit:** a call that recurses too deep reports an error instead of overflowing the kernel stack (6.5.2). Ribbit code may use 48 KB of the kernel's 64 KB stack, which is about 35 nested calls of a small function. Reading code counts against the same 48 KB, so code nested too deeply to read, such as dozens of blocks inside each other, reports the same error.

```csharp
delegate Int64 Op(Int64 a, Int64 b);
Op add = (a, b) => a + b;
Func<Int64, Int64> square = x => x * x;
Action<String> say = s => Console.WriteLine(s);

IEnumerable<Int64> Countdown(Int64 from)
{
    while (from > 0) {
        yield return from;
        from -= 1;
    }
}
```

A lambda can use variables from around it, and it keeps them alive.

## 9. Classes, structs, interfaces, enums, generics and arrays

```csharp
interface IShape { Double Area(); }

abstract class Shape : IShape
{
    public abstract Double Area();
    public virtual String Name() { return "shape"; }
}

sealed class Circle : Shape
{
    private readonly Double radius;
    public static Int64 Count;

    public Circle(Double r) { radius = r; Count += 1; }
    public override Double Area() { return 3.14159 * radius * radius; }
    public override String Name() { return "circle"; }
}

struct Vector
{
    public Int64 X;
    public Int64 Y;
    public static Vector operator +(Vector a, Vector b) { ... }
}

enum Color : Byte { Black, Blue, Green, Cyan, Red }

class Stack<T>
{
    private List<T> items = new List<T>();
    public Void Push(T item) { items.Add(item); }
}

T Larger<T>(T a, T b) where T : IComparable<T>
{
    if (a.CompareTo(b) > 0) { return a; }
    return b;
}

Int64[] a = new Int64[10];     // a.Length is 10, and indexes are checked
Int64[] b = { 1, 2, 3 };
```

- **`struct` vs `class`:** a `struct` is a value that's copied when assigned. A `class` is a shared object made with `new`.
- **Defaults:** members are `private` unless you mark them otherwise, as in C#.
- **Inheritance:** a class has one base class and can implement many interfaces.
- **No properties (`get`/`set`).** Classes have fields and methods. Built-in types can still offer read-only members like `s.Length`.
- **Library types:** `List<T>`, `Dictionary<K, V>`, `IEnumerable<T>`, `Func<...>`, `Action<...>`, `IComparable<T>` and `Exception`

## 10. Unsafe code, namespaces and memory

```csharp
unsafe Void PutChar(Int64 col, Int64 row, Char c)
{
    volatile Byte* vga = (Byte*)0xB8000;
    Int64 cell = (row * 80 + col) * 2;
    vga[cell] = (Byte)c;
    vga[cell + 1] = 0x0F;      // white on black
}
```

- **Pointers:** `T*`, `&x`, `*p`, `p->x`, `p[i]` and pointer casts are only allowed inside `unsafe` blocks and functions.
- **`fixed`:** `fixed (Char* p = name) { }` keeps a String alive while you hold a raw pointer into it.
- **`volatile`** also works on what a pointer points at, so hardware registers are always really read and written. C# doesn't allow that.
- **Ring 0:** all code still runs in ring 0, as in TempleOS. `unsafe` and `private` are rules the compiler checks, not protection the CPU enforces.

**Namespaces.** `namespace Gfx { ... }` groups names, which you then use as `Gfx.Line(...)`. `using Gfx;` lets you write `Line(...)`, and a `using` typed at the prompt lasts for the session. A file can pull in another with `#include "gfx.rbt"` (6.6.3), which is a directive rather than a keyword.

Built-in namespaces, first draft (they grow with the roadmap):

| Namespace | Functions |
|---|---|
| `Console` | `Write`, `WriteLine`, `Clear` |
| `Speaker` | `Beep(hz, ms)`, with 20 to 20000 Hz and up to 10000 ms; `Play(tune)`, with notes such as `"C4 E4 G4:500 R C5"` |
| `Timer` | `Uptime` (milliseconds, as an Int64), `Sleep(ms)`, up to 10000 ms |
| `File` | `ReadAllText`, `WriteAllText`, `Exists` |
| `Math` | |

Until namespaces are built (6.6.4), the interpreter treats `Console.WriteLine` as one dotted name.

**Memory: reference counting.** Strings, arrays, objects and lambdas each carry a hidden count of how many references point at them. They're freed the moment that count reaches 0. There's no garbage collector and so no GC pauses, which suits an OS. `using (...) { }` frees things like open files at a known moment.
- **The catch:** two objects that point at each other are never freed. For example, a Window that holds its Button while the Button holds its Window. See ⚑ 6.7.9.

## 11. Build order

Step IDs are from [ROADMAP.md](ROADMAP.md).

| When | What |
|---|---|
| M2: 6.3–6.5 | Typed declarations with `Int64`, `Boolean` and `Void`, plus `var`, `const`, `true` and `false` (6.3.5); `&& \|\| !`, bitwise operators, shifts and hex literals (6.3.1); `String`, `Char` and `$"..."` (6.3.3); blocks, `if`, `while`, `do`, `for`, `break` and `continue` (6.4); the `;` rule and `assert` (6.4.4); functions and recursion (6.5); built-ins named `Console.WriteLine` and so on (6.5.3) |
| M4: 6.6 | argparse-style commands (6.6.1), running files and `#include` (6.6.3), namespaces, `using`, and the `extern` library file (6.6.4) |
| M8: 6.7 | Arrays, `struct`, `enum`, `switch` and `foreach` (6.7.1); `unsafe` (6.7.2); type checking (6.7.3); classes (6.7.4); interfaces and inheritance (6.7.5); delegates and lambdas (6.7.6); generics and `yield` (6.7.7); exceptions and `using` blocks (6.7.8); `lock` and `async`/`await`, which need tasks from M5 (6.7.10) |
| M8: 6.8 | Compiling all of it to x86-64 |

## 12. Still to decide

| What | Needed by |
|---|---|
| How generics compile: one copy per type, or one shared copy | 6.7.7 / 6.8 |
| Reference-count cycles: weak references or a cycle collector | 6.7.9 |
| What `async`/`await` means on top of FroggeOS tasks. C#'s `Task` type also clashes with the kernel's Task (2.6). | 6.7.10 |
| How compiled code unwinds the stack when an exception is thrown | 6.8 |
