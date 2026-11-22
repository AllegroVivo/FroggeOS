# FroggeOS Roadmap

**Status (2026-11-22):** milestones 0 and 1 are done, and work on M2 has started. FroggeOS boots through its own stage 1 and stage 2 into long mode, then runs a Rust kernel. The kernel has VGA text output copied to a serial port, its own GDT and TSS, named handlers for all 32 CPU exceptions (a double fault gets its own stack), a guard page under the stack, the PIC, a 100 Hz timer, the CMOS clock and the PC speaker with a note player, a PS/2 keyboard with arrow keys, Caps Lock, Ctrl and Alt (Ctrl+C throws away a typed line or stops running code), a free-list heap, a shell whose arrow keys edit the line and recall earlier ones, and a `selftest` command that `build.ps1 -SelfTest` runs with no window. The shell runs Ribbit, which lives in its own crate with `cargo test` tests and so far has Int64, Boolean, Char and String values, typed declarations (`Int64`, `Boolean`, `var`, `const`), block scope, comparisons, `&&` `||` `!`, bitwise operators, shifts and hex literals, `if`/`else`, `while`, `do`/`while`, `for`, `break` and `continue`, `assert`, shorthand assignment (`+=`, `++`), functions with recursion, built-ins like `Console.WriteLine` and `Speaker.Beep`, `$"..."` interpolation, multi-line input, a `^` under each syntax error, and C#'s rule that every statement ends with `;`. M1's demo works (editing and recalling lines, `date`, a beep, and a clean report for a stack overflow), and so does M2's: a recursive `Fib()` typed at the prompt. The kernel is 144 KB of its 256 KB budget.

**How to read this:** every item has an ID so we can say "next is 2.2.2" and break any item down further when we reach it.
- ✅ means done.
- ⚑ marks a design decision, made when we reach it rather than now.
- ☆ marks something optional.

---

## Part A: The tree (organised by system)

### 1. Boot
- **1.1 Stage 1 (MBR)** ✅
  - 1.1.1 Set up segments and stack, save the boot drive ✅
  - 1.1.2 Load stage 2 with BIOS int 13h ✅
  - 1.1.3 ☆ Add an MBR partition table. Some real PCs won't boot a USB stick without one.
- **1.2 Stage 2** ✅
  - 1.2.1 Real mode → protected mode → long mode, with a GDT and the first 1 GB identity-mapped using 2 MB pages ✅
  - 1.2.2 Load the kernel using CHS addressing, one sector at a time ✅
- **1.3 Move from floppy to hard disk**
  - 1.3.1 Build a hard-disk image (QEMU's `-hda`) instead of a 1.44 MB floppy
  - 1.3.2 Check that the BIOS has LBA extensions (int 13h, AH=41h)
  - 1.3.3 Read many sectors per call with AH=42h and a Disk Address Packet. This removes the cylinder/head/sector maths.
  - 1.3.4 Have build.ps1 work out the sector counts and pass them to NASM with `-D`. You'd no longer have to keep the two constants matched by hand.
- **1.4 A bigger kernel** (stage 2 loads up to 256 KB at 0x10000, and there's room up to the stack's guard page at 0x7F000, about 444 KB; needed once the kernel outgrows that)
  - 1.4.1 Load the kernel in chunks into a buffer below 1 MB, then copy each chunk above 1 MB. Real mode can't write past 1 MB directly.
  - 1.4.2 Move the stack (now at 0x90000) and the heap (hard-coded to 1–2 MB) out of the kernel's new space
- **1.5 Pass information to the kernel**
  - 1.5.1 Collect the BIOS memory map (int 15h, E820) in stage 2
  - 1.5.2 Define a `BootInfo` struct with the same layout in assembly and Rust: memory map, boot drive, and later the framebuffer
  - 1.5.3 Pass its address in RDI so `_start(boot_info: &BootInfo)` receives it. This follows the System V calling convention, which the `extern "C"` `_start` already uses.
- **1.6 Graphics mode at boot** (only if 4.1 picks this route)
  - 1.6.1 List the video modes with int 10h, 4F00h/4F01h, and pick one
  - 1.6.2 Set it with 4F02h, using a linear framebuffer, and store the address, pitch and bits per pixel in `BootInfo`
- **1.7 Robustness** ☆
  - 1.7.1 Check that A20 really turned on, and fall back to the keyboard-controller method if not
  - 1.7.2 Use CPUID to check the CPU supports long mode, and show a friendly error if it doesn't
  - 1.7.3 Boot on a real PC from a USB stick (needs 1.1.3)

### 2. Kernel core
- **2.1 Output and diagnostics**
  - 2.1.1 VGA writer, `print!`/`println!`, panic handler ✅
  - 2.1.2 Serial port (COM1) driver that copies all output to QEMU's `-serial stdio`. You get logs you can copy, and they survive a crash. ✅
  - 2.1.3 A `log!` macro with levels (info, warn, error) that writes to serial only
  - 2.1.4 ☆ A crash screen showing all registers plus a stack backtrace. The backtrace needs `-C force-frame-pointers=yes`.
- **2.2 CPU setup**
  - 2.2.1 Give the kernel its own GDT, written in Rust, instead of using the one inside stage 2 ✅
  - 2.2.2 Add a TSS with an Interrupt Stack Table entry. A double fault (for example, a stack overflow) then gets a fresh stack instead of causing a triple fault and reboot. ✅
  - 2.2.3 Detect CPU features with CPUID and add a `cpu` command
  - 2.2.4 ⚑ Floating point. Rust currently does floats in software on this target, which works but is slow. Turning on SSE gives hardware floats, but then the task switcher must also save FPU state.
- **2.3 Interrupts**
  - 2.3.1 IDT with handlers for exceptions 0, 3, 6, 8, 13 and 14 ✅
  - 2.3.2 PIC remap and IRQ handling ✅
  - 2.3.3 Handlers with names for all 32 exception vectors ✅
  - 2.3.4 Handle the PIC's spurious IRQ 7 and IRQ 15
  - 2.3.5 An IRQ registration API, so each driver installs its own handler instead of editing interrupts.rs
  - 2.3.6 Replace the PIC with the Local APIC and I/O APIC (needs 2.7.1)
- **2.4 Memory**
  - 2.4.1 Free-list heap that merges neighbouring free blocks ✅
  - 2.4.2 Read the E820 map from 1.5 and add a `mem` command
  - 2.4.3 Physical frame allocator: a bitmap of 4 KB frames, with the kernel, BIOS and heap regions marked as used
  - 2.4.4 Page-table code in Rust: walk PML4 → PDPT → PD → PT, map and unmap a page, flush the TLB with `invlpg`
  - 2.4.5 Map device memory above 1 GB, such as the framebuffer and the APIC at 0xFEE00000. The boot page tables only cover the first 1 GB.
  - 2.4.6 ⚑ Address-space model (see the decisions table)
  - 2.4.7 Size the heap from how much RAM is really there, or let it grow by asking for more frames
  - 2.4.8 ☆ Unmapped guard pages under each stack, so an overflow causes a clean page fault instead of silent corruption. The boot stack has one ✅ (it overflows into a reported double fault). The double-fault stack doesn't yet, and task stacks (2.6) will need them too.
- **2.5 Time**
  - 2.5.1 PIT at 100 Hz, a tick counter, `uptime` ✅
  - 2.5.2 `sleep(ms)`: wait with `hlt` for now; it becomes a real sleep once tasks exist (2.6.5) ✅
  - 2.5.3 Read the date and time from the CMOS real-time clock, and add a `date` command ✅
  - 2.5.4 ☆ Calibrate the TSC against the PIT for fine-grained timing (profiling, frame rates)
  - 2.5.5 A Local APIC timer on each core (comes with 2.7)
- **2.6 Tasks (multitasking)**
  - 2.6.1 A Task struct: id, name, state, saved stack pointer and its own stack
  - 2.6.2 A context switch in assembly: push the callee-saved registers, swap RSP, pop them. It's a naked function, like the exception stubs.
  - 2.6.3 Spawn a task from a Rust closure, and clean up when a task exits
  - 2.6.4 ⚑ Scheduling style (see the decisions table)
  - 2.6.5 Sleeping and blocked states: a sleep queue that the timer wakes, and an idle task that runs `hlt`
  - 2.6.6 A mutex and wait queues that block a task instead of spinning
  - 2.6.7 Message queues: keyboard and mouse events go to the focused task. Windows are built on this later.
  - 2.6.8 Shell commands: `tasks`, `kill <id>`, and running a command in the background
- **2.7 Multi-core (SMP)**
  - 2.7.1 ACPI: find the RSDP, then parse the RSDT/XSDT and MADT to list the CPU cores and the I/O APIC
  - 2.7.2 Turn on the Local APIC and route IRQs through the I/O APIC (this is 2.3.6)
  - 2.7.3 Start the other cores with INIT-SIPI-SIPI. Each one runs a real-mode trampoline up to long mode, so stage 2 gets written again.
  - 2.7.4 Per-core data (current task, stacks), reached through the GS register
  - 2.7.5 The TempleOS model: core 0 runs the user interface, and the other cores take jobs from a queue
  - 2.7.6 Check that shared state (heap, scheduler) is safe with several cores. `SpinLock` is already atomic, so it's a good base.

### 3. Device drivers
- **3.1 Keyboard**
  - 3.1.1 IRQ 1, scancode ring buffer, US layout, Shift ✅
  - 3.1.2 Caps Lock, Ctrl and Alt (Ctrl+C to cancel) ✅
  - 3.1.3 Extended scancodes (the 0xE0 prefix): arrows, Home/End, Delete, PgUp/PgDn ✅
  - 3.1.4 Key events (press, release, modifiers) as well as characters. Games and the editor need these.
  - 3.1.5 ☆ Keyboard LEDs
- **3.2 Serial port:** output first (2.1.2) ✅; input later ☆ so you can type into FroggeOS from your own terminal
- **3.3 PC speaker** (TempleOS's only sound device)
  - 3.3.1 A beep at a chosen frequency, using PIT channel 2 and port 0x61. To hear it, QEMU needs `-audiodev` and `pcspk-audiodev`. ✅
  - 3.3.2 A note player (`play C4 E4 G4:500 R C5` at the prompt, `Speaker.Play` in Ribbit) ✅
- **3.4 PS/2 mouse**
  - 3.4.1 Enable the 8042 controller's second (aux) port and IRQ 12
  - 3.4.2 Turn its 3-byte packets into movement (dx, dy) and button states
  - 3.4.3 Send mouse events to the message queues (2.6.7)
- **3.5 PCI**
  - 3.5.1 Scan the bus through ports 0xCF8/0xCFC, and add a `pci` command
  - 3.5.2 Read each device's BARs (base address registers) to find its memory and I/O ports
- **3.6 Disk**
  - 3.6.4 RAM disk, so the filesystem can be tested before the disk driver works (built first)
  - 3.6.1 ATA PIO driver (primary IDE, LBA28): identify the drive and read sectors
  - 3.6.2 Write sectors and flush the drive's cache
  - 3.6.3 A `BlockDevice` trait, so the filesystem doesn't care which device it's on
  - 3.6.5 ☆ Interrupt-driven or DMA transfers; AHCI (SATA)

### 4. Graphics
- **4.1** ⚑ Display route and resolution (see the decisions table)
- **4.2 Framebuffer basics**
  - 4.2.1 Map the framebuffer (2.4.5); `put_pixel`, `fill_rect`, `clear`
  - 4.2.2 Lines (Bresenham's algorithm), rectangle outlines, circles
  - 4.2.3 Draw into a back buffer in RAM, then copy it to the screen so nothing flickers. ☆ Redraw only the parts that changed.
  - 4.2.4 Copy bitmaps onto the screen, with transparency
- **4.3 Text on pixels**
  - 4.3.1 Embed an 8×8 or 8×16 bitmap font as a constant array
  - 4.3.2 `draw_char` and `draw_str` with colours
  - 4.3.3 A framebuffer console. `print!` keeps working, just with a new backend: a `Console` trait implemented once for VGA text and once for the framebuffer. Good practice with Rust traits.
- **4.4** A mouse cursor drawn over the screen (needs 3.4)
- **4.5** The TempleOS 16-colour palette as named constants
- **4.6** ☆ Sprites: a small image format, plus an editor for it (7.5.4)

### 5. Storage and files
- **5.1** ⚑ Disk layout (see the decisions table)
- **5.2 Filesystem design** (written up as a short spec first)
  - 5.2.1 ⚑ Own filesystem or FAT32 (see the decisions table)
  - 5.2.2 On-disk structures: a superblock (filesystem header), an allocation bitmap, and directory entries holding name, size, first block and date
- **5.3 Host tool `mkimage`:** a normal Rust program using std, run on Windows, that packs a folder into disk.img. The first Rust program outside the kernel, and build.ps1 calls it.
- **5.4 Reading files in the kernel**
  - 5.4.1 Mount: read and check the superblock
  - 5.4.2 List a directory and look up paths like `/docs/hello.txt`
  - 5.4.3 Read a whole file into a `Vec<u8>`
  - 5.4.4 Shell commands: `ls`, `cat`, `cd`, `pwd`
- **5.5 Writing files**
  - 5.5.1 Allocate and free blocks through the bitmap
  - 5.5.2 Create, overwrite and delete files; `mkdir` and `rmdir`
  - 5.5.3 Shell commands: `cp`, `rm`, `mkdir`
  - 5.5.4 ☆ Write things in an order that keeps a power cut from corrupting directories
- **5.6** ⚑ File API shape (see the decisions table)
- **5.7** ☆ Stage 2 finds `KERNEL.BIN` by name instead of at fixed sectors

### 6. The language, Ribbit (the heart of TempleOS)
In TempleOS the shell, scripts and programs are all one language, HolyC.
- **6.0** Integer calculator: tokenizer, recursive-descent parser, syntax tree, evaluator ✅ (this is the seed of the whole language)
- **6.1** Syntax, typing and how code runs: decided, see [LANGUAGE.md](LANGUAGE.md) ✅
- **6.2** Move the language into its own `no_std` library crate, so `cargo test` runs it on Windows without QEMU ✅ (the `ribbit` crate)
- **6.3 Expressions**
  - 6.3.1 Comparisons, `&&` `||` `!`, bitwise operators, shifts, `%`, hex literals ✅
  - 6.3.2 Variables and assignment that persist between shell lines (a symbol table), plus a `vars` command ✅
  - 6.3.3 Strings and characters: `String` with `+`, `==` and the escapes `\n \t \\ \" \0` ✅, `$"..."` interpolation ✅, a 1-byte `Char`, `\xNN`, `s[i]` and `.Length` ✅ (ASCII only for now: `\x00` to `\x7F`)
  - 6.3.4 Error messages with line and column, and a caret under the problem: syntax errors ✅, errors found while running
  - 6.3.5 Typed declarations (`Int64 x = 5;`, `var`, `const`) and a strict `Boolean` type with `true`/`false`. Assigning to an undeclared name becomes an error. ✅
- **6.4 Statements**
  - 6.4.1 Statement lists, `;` and `{ }` blocks, each with its own scope ✅
  - 6.4.2 `if`/`else`, `while`, `for`, `break`/`continue` ✅
  - 6.4.3 Multi-line input: the shell keeps reading until the braces balance ✅
  - 6.4.4 Make `;` required (LANGUAGE.md §6), and add `do`/`while` and `assert` ✅
  - 6.4.5 Shorthand assignment: `x += e` (and `-= *= /= %= &= |= ^= <<= >>=`), `x++` and `x--` ✅
- **6.5 Functions**
  - 6.5.1 Definitions, parameters, return values, recursion ✅
  - 6.5.2 Call frames and local scopes, plus a recursion limit so `f(){f();}` reports an error instead of overflowing the kernel stack ✅ (each line may use 48 KB of the 64 KB stack, both while it's read and while it runs)
  - 6.5.3 Built-in functions written in Rust, with .NET-style names: `Console.Write`, `Console.WriteLine`, `Console.Clear`, `Timer.Uptime`, `Speaker.Beep`, `Speaker.Play` and `Timer.Sleep` ✅, …
- **6.6 Commands, scripts and the library**
  - 6.6.1 A command table, argparse-style. Each command declares its arguments (positional, `-f`/`--flag`, options with values), and one shared parser checks them and generates `help <command>`. A line whose first word is a command runs it; anything else is code.
  - 6.6.2 Make kernel features available as built-ins: files (5), graphics (4), sound (3.3), tasks (2.6)
  - 6.6.3 Run a `.rbt` source file, support `#include`, and run a startup script at boot
  - 6.6.4 Namespaces and `using`, and the built-in library as a FroggeOS source file of `extern` declarations
- **6.7 Data and types** (the C#-style features in LANGUAGE.md)
  - 6.7.1 Arrays (`Int64[]`), `struct`, `enum`, `switch` and `foreach`
  - 6.7.2 Raw pointers inside `unsafe`: `T*`, `&`, `*`, `->`, `fixed`, `sizeof`, `volatile`
  - 6.7.3 A type-checking pass
  - 6.7.4 Classes: `new`, `null`, `this`, constructors, `static`, `readonly`, `public`/`private`/`protected`, freed by reference counting
  - 6.7.5 Interfaces and inheritance: `virtual`, `override`, `abstract`, `sealed`, `base`, `is`, `as`, `typeof`
  - 6.7.6 Delegates and `=>` lambdas, plus `params`, `ref`, `out` and `operator`
  - 6.7.7 ⚑ Generics (`List<T>`, `Dictionary<K, V>`, your own `class Stack<T>`, `where`), then `yield` and `IEnumerable<T>`
  - 6.7.8 Exceptions (`try`/`catch`/`finally`/`throw`) and `using (...) { }` blocks
  - 6.7.9 ⚑ Reference-count cycles (see the decisions table)
  - 6.7.10 ⚑ `lock`, then `async`/`await` (needs tasks, 2.6)
- **6.8 Compiling for speed**
  - 6.8.1 Compile to bytecode and run it on a stack-based VM. Much faster than walking the syntax tree.
  - 6.8.2 An x86-64 code generator: write machine-code bytes into memory and call them like a function. This is what HolyC does.
  - 6.8.3 A calling convention so compiled code and Rust built-ins can call each other
  - 6.8.4 ☆ Compile whole files when they're loaded
- **6.9** ☆ A disassembler for the code generator, and a debugger with breakpoints (it uses the existing `int3` handler)

### 7. Shell, user interface and apps
- **7.1 Text shell** ✅ (help, clear, echo, uptime, date, beep, play, history, heap, selftest, vars, lang, calculator)
  - 7.1.1 Line editing: move the cursor left and right, insert mid-line, Delete (needs 3.1.3) ✅
  - 7.1.2 Up and down arrows recall history ✅
  - 7.1.3 Tab completion
  - 7.1.4 Colours for the prompt and errors
  - 7.1.5 ☆ Scrollback
- **7.2 Text-mode editor** (the first editor inside the OS, before graphics)
  - 7.2.1 A text buffer: a `Vec` of lines, or a gap buffer
  - 7.2.2 Moving the cursor, inserting and deleting, scrolling through files longer than the screen
  - 7.2.3 Open and save files (5)
  - 7.2.4 F5 runs the file being edited (6.6.3). That's the TempleOS loop: write, run and fix without leaving the OS.
- **7.3 Window manager** (needs 2.6, 4 and 3.4)
  - 7.3.1 A Window struct owned by a task: position, size, title, its own pixel buffer
  - 7.3.2 Draw the desktop, then the windows from back to front
  - 7.3.3 Focus, stacking order, bring a window to the front on click
  - 7.3.4 Drag the title bar to move, a close button, ☆ resizing
  - 7.3.5 Send input to the focused window's task (2.6.7)
  - 7.3.6 Terminal windows, so several shells can run at once
- **7.4 Rich documents** ☆ (TempleOS's DolDoc idea)
  - 7.4.1 ⚑ A small markup: colours, links, embedded sprites
  - 7.4.2 One renderer shared by shell output, help pages and the editor
  - 7.4.3 Clickable links that run code or open files
- **7.5 Apps and fun** (do these whenever you want a break)
  - 7.5.1 Info commands: `date`, `mem`, `cpu`, `pci`, `tasks`
  - 7.5.2 A music player (built on 3.3.2)
  - 7.5.3 A small game of our own design: text mode first, graphical later
  - 7.5.4 A paint program or sprite editor
  - 7.5.5 A system monitor window
  - 7.5.6 Demos: 3D wireframes, fractals (these want hardware floats, 2.2.4)

### 8. Tooling and testing (cuts across every area; do the first three early)
- **8.1** Serial logging (this is 2.1.2) ✅
- **8.2** Host-side unit tests: move pure logic (language, heap algorithm, filesystem format) into `no_std` library crates and test them with `cargo test` on Windows
- **8.3** In-kernel self-tests: a `selftest` command that prints PASS or FAIL over serial, then closes QEMU with a status code through the `isa-debug-exit` device. This automates the boot checks now done by hand. Rust's built-in custom test framework needs nightly Rust, but this approach works on stable. ✅
- **8.4** Debugging with GDB: QEMU `-s -S`, plus keeping the ELF file next to the flat binary so GDB has symbols
- **8.5** ☆ Replace build.ps1 with a Rust `xtask` build tool
- **8.6** ~~Merge `first-boot` into `main` at each milestone~~ No longer needed: `first-boot` became `main` when the project went on GitHub ✅

**Parked on purpose:** USB (huge; the BIOS and QEMU emulate PS/2 for keyboard and mouse), networking (TempleOS has none by design), UEFI, sound cards, user mode and memory protection between programs.

---

## Part B: Build order (milestones)

| # | Milestone | Steps | The demo when it's done |
|---|---|---|---|
| M0 ✅ | Boot to a shell | 1.1, 1.2, 2.1.1, 2.3.1–2, 2.4.1, 2.5.1, 3.1.1, 6.0, 7.1 | Where we are |
| M1 ✅ | Solid foundations | 2.1.2 → 8.3 → 2.2.1–2 → 2.3.3 → 3.1.2–3 → 7.1.1–2 → 2.5.3 → 3.3.1 | Edit a line with the arrows, recall history, `date`, a beep. A stack overflow gives a clean report instead of a reboot. |
| M2 | Language v1 | 6.1 → 6.2 → 6.3 → 6.4 → 6.5 | Define a recursive `Fib()` at the prompt and call it |
| M3 | Know the machine | 1.3 → 1.5 → 2.4.2–5 → 2.4.7 → 1.4 when needed | `mem` shows the real RAM, and the heap uses all of it |
| M4 | Files and scripts | 3.6.4 → 3.6.1–3 → 5.1–5.6 → 6.6 → 7.2 | Write a program in the editor, save it, reboot, and run it |
| M5 | Multitasking | 2.6 → 2.5.2 | A background task counts while you keep typing |
| M6 | Graphics and mouse | 4.1 → 1.6 or 3.5 → 4.2–4.5 → 3.4 | The shell drawn in pixels, with a mouse cursor |
| M7 | Windows | 7.3 | Two shells side by side in draggable windows |
| M8 | The compiler | 6.7 → 6.8 → 2.2.4 | Our own code compiled to x86-64 and running |
| M9 | Multi-core | 2.7 | `tasks` shows jobs running on four cores |
| M10 | Polish and fun | 7.4, 7.5, 1.7 | Games, music, help docs, a boot on a real PC |

M2 and M3 don't depend on each other, so they can be swapped. The language comes first because the calculator gives momentum, and there's 85 KB of room left.

---

## Decisions to make along the way

None of these need an answer up front. Each one gets decided when its milestone comes up.

| ID | Decision | Needed by | Recommendation |
|---|---|---|---|
| 6.1 | Language syntax, typing and how it runs | M2 | ✅ Decided: C#-style with CLR type names, written up in LANGUAGE.md. Interpret first, compile in M8. |
| 6.2 | The language's name | M2 | ✅ Decided: Ribbit. Scripts are `.rbt` files, and the crate is `ribbit`. |
| 2.4.6 | Address-space model | M3 | One identity-mapped address space, everything in ring 0, like TempleOS. Much simpler, and Rust's safety helps make up for it. |
| 5.1 / 5.2.1 | Disk layout and filesystem | M4 | A fixed layout (boot, kernel, then filesystem) with our own simple RedSea-like filesystem. `mkimage` covers getting files in from Windows, so FAT isn't needed. |
| 5.6 | File API | M4 | Whole-file `read_file`/`write_file`, like TempleOS |
| 2.6.4 | Scheduling | M5 | Cooperative `yield()` first, like TempleOS. Timer preemption can be added later. |
| 4.1 | Display route | M6 | Set a VESA BIOS (VBE) mode in stage 2, with 32-bit colour. It fits the hand-built bootloader and works on real PCs, unlike the QEMU-only Bochs adapter. |
| 2.2.4 | Hardware floats | M8 or demos | Keep software floats until something is too slow |
| 6.7.2 | Raw pointers in the language | M8 | ✅ Decided: yes, inside `unsafe` blocks, as in C# |
| 6.7.7 | How generics compile | M8 | Shared code for reference types and one copy per value type, as .NET does |
| 6.7.9 | Reference-count cycles | M8 | Weak references first; a cycle collector only if leaks become a problem |
| 6.7.10 | What `async`/`await` means on FroggeOS tasks | M8 | Each awaited call runs as its own task, and `await` yields until it finishes. That fits cooperative scheduling (2.6.4). |
| 6.8 | Exception unwinding in compiled code | M8 | Walk the frame pointers that 2.1.4 already wants |
