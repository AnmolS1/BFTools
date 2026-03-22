# BFTools — Complete User Guide

BFTools is a VS Code extension that turns Brainfuck into a first-class development experience. This guide walks through every feature from installation to AOT compilation.

---

## Table of Contents

1. [Getting Started](#getting-started)
2. [Syntax Highlighting](#syntax-highlighting)
3. [Formatting](#formatting)
4. [Linting and Diagnostics](#linting-and-diagnostics)
5. [Running Programs](#running-programs)
6. [Debugging](#debugging)
7. [Test Runner](#test-runner)
8. [testDebug Mode](#testdebug-mode)
9. [AOT Precompilation](#aot-precompilation)
10. [Configuration Reference](#configuration-reference)
11. [VS Code Commands Reference](#vs-code-commands-reference)
12. [Troubleshooting](#troubleshooting)

---

## Getting Started

### Install the Extension

**From the Marketplace:**

1. Open VS Code
2. Press `Ctrl+Shift+X` (or `Cmd+Shift+X` on Mac) to open Extensions
3. Search for **"Brainfuck"** by *Ponderance*
4. Click **Install**

Or via the command line:
```bash
code --install-extension Ponderance.brainfuck-vscode
```

**From Source:**

```bash
git clone https://github.com/AnmolS1/BFTools
cd BFTools
cargo build --workspace --release
npm install && npm run compile
npx vsce package
code --install-extension brainfuck-vscode-*.vsix
```

### Verify the Installation

1. Create a file named `hello.bf` with any content, e.g., `+.`
2. Open it in VS Code
3. The status bar should show `Brainfuck` as the language
4. BF operators should be colorized — if they are, the extension is working

---

## Syntax Highlighting

BFTools registers `.bf` files as the `brainfuck` language and applies a TextMate grammar for colorization.

**What gets colorized:**

| Token | Color (varies by theme) |
|-------|------------------------|
| BF operators (`+`, `-`, `<`, `>`, `.`, `,`, `[`, `]`) | Keyword / operator color |
| `//` line comments | Comment color |
| `/* */` block comments | Comment color |

**Bracket matching:** VS Code's built-in bracket colorizer highlights matching `[` / `]` pairs when your cursor is on one of them.

**Auto-closing:** Typing `[` automatically inserts the matching `]`.

---

## Formatting

The formatter produces a canonical representation that is consistent and idempotent (running it twice gives the same result).

### How to Trigger

| Method | Action |
|--------|--------|
| Keyboard | `Shift+Alt+F` (Windows/Linux) / `Shift+Option+F` (Mac) |
| Command Palette | `Brainfuck: Format Document` |
| On save | Enable `"editor.formatOnSave": true` in VS Code settings |

### What the Formatter Does

**1. Instruction grouping** — Consecutive operators on the same cell are placed together on one line. A space separates groups for different cells:
```bf
// Before
+ + + > + + < -
// After
+++ >++ <-
```

**2. Inline loops** — A loop whose body fits within 80 characters stays on one line:
```bf
[-]         // zero-cell idiom
[->+<]      // move cell right
```

**3. Block loops** — Longer loops are broken across multiple lines. The body is indented by one tab per nesting level:
```bf
[
    >>>++++[>++++++++<-]>
    <<<<-
]
```

**4. Indentation** — Uses tabs (one per nesting level). Never spaces.

**5. Inline comments** — A `//` comment following instructions stays on the same line (separated by two spaces):
```bf
>++  // set cell 1 to 2
```

**6. Block comments** — A standalone comment line stays at the current indentation level:
```bf
[
    // This comment is inside the loop
    -
]
```

**7. Trailing newline** — Output always ends with exactly one `\n`.

---

## Linting and Diagnostics

Diagnostics update in real time as you type. Problems appear as colored squiggles in the editor and in the **Problems** panel (`Ctrl+Shift+M`).

Hover over any squiggle to see the rule code and message.

### Rules

#### BF001 — Invalid Character (Error)

**Triggered by:** Any character that is not a BF operator, whitespace, or part of a comment.

```bf
int x = 5;  // BF001: 'i', 'n', 't', etc. are invalid
```

**Fix:** Remove the invalid character, or wrap it in a comment (`//` or `/* */`).

---

#### BF002 — Unmatched `[` (Error)

**Triggered by:** An opening bracket `[` with no corresponding `]`.

```bf
+[->+<   // BF002: '[' has no matching ']'
```

**Fix:** Add the missing `]` at the correct position.

---

#### BF003 — Unmatched `]` (Error)

**Triggered by:** A closing bracket `]` with no corresponding `[`.

```bf
+->+<]   // BF003: ']' has no matching '['
```

**Fix:** Add the missing `[` or remove the orphaned `]`.

---

#### BF004 — Empty Loop (Warning)

**Triggered by:** A `[]` construct with no instructions inside.

```bf
[]   // BF004: loop body is empty — this is a no-op or infinite loop
```

**Fix:** Either add instructions inside the loop, or remove it if it was a mistake.

---

#### BF005 — Redundant Operations (Hint)

**Triggered by:** Adjacent `+-` or `-+` that cancel each other out.

```bf
+++-   // BF005: the '+' and '-' cancel; equivalent to '++'
```

**Fix:** Simplify to the net effect: `++` in the example above.

---

#### BF006 — Dead Code After `[-]` (Warning)

**Triggered by:** Instructions that follow a `[-]` (zero-cell) pattern at the same nesting level, before any write (`,`) or pointer move.

```bf
[-]++++   // BF006: '[-]' zeros the cell; the '++++ ' that follow are redundant
```

**Fix:** Remove the dead code or restructure so the pointer moves before the subsequent instructions.

---

## Running Programs

BFTools provides three execution modes with increasing performance. All modes read from the VS Code terminal's standard input and write to standard output.

### Execution Modes Compared

| Mode | Command | How it works | Best for |
|------|---------|-------------|----------|
| **Interpreter** | `Brainfuck: Run Brainfuck Program` | Tree-walking bytecode interpreter | Quick runs, learning, compatibility |
| **JIT** | `Brainfuck: Run with JIT` | Cranelift compiles entire program to native code before running | CPU-intensive programs |
| **Auto-JIT** | `Brainfuck: Run with Auto JIT` | Starts as interpreter; hot loops (executed ≥ 3 times) are JIT-compiled on demand | Balance of startup speed and throughput |

### Running from VS Code

1. Open a `.bf` file
2. Open the Command Palette (`Ctrl+Shift+P`)
3. Type the command name and press Enter
4. A terminal opens and the program starts immediately

If your program reads from stdin (`,` operator), the terminal is interactive — type input and press Enter.

### Running from the CLI

```bash
# Interpreter (default)
bf-interpreter hello_world.bf

# JIT mode
bf-interpreter --jit hello_world.bf

# Auto-JIT mode
bf-interpreter --auto-jit hello_world.bf

# Force interpreter (if binary was built with JIT feature)
bf-interpreter --no-jit hello_world.bf
```

**Exit codes:** `0` = normal exit, `2` = runtime error (e.g., pointer out of bounds).

---

## Debugging

BFTools implements the [Debug Adapter Protocol (DAP)](https://microsoft.github.io/debug-adapter-protocol/), giving you VS Code's full debug UI for Brainfuck.

### Starting a Debug Session

**Quick start:** Press `F5` with a `.bf` file active. VS Code will use the default launch configuration.

**Custom configuration** (`.vscode/launch.json`):

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "type": "brainfuck",
      "request": "launch",
      "name": "Debug Brainfuck",
      "program": "${file}"
    }
  ]
}
```

The `program` field accepts any absolute path or VS Code variable like `${file}` (active file) or `${workspaceFolder}/myprog.bf`.

### Setting Breakpoints

Click in the **gutter** (left margin) next to a line number to toggle a breakpoint. A red dot appears.

**How breakpoints resolve:**
- Lines with BF instructions → resolved, solid red dot
- Lines with only whitespace or comments → unverified, hollow red dot (the debugger will not stop here)
- The debugger stops *before* executing the first instruction on the breakpoint line

### Stepping Through Code

Once stopped at a breakpoint:

| Action | Keybinding | What it does |
|--------|-----------|--------------|
| **Continue** | `F5` | Run until the next breakpoint or program end |
| **Next (Step Over)** | `F10` | Execute exactly one BF instruction, then pause |
| **Step Out** | `Shift+F11` | Run until the current loop body exits, then pause |

> **Tip:** Use *Step Out* when you're inside a loop and want to continue past it without stepping every iteration.

### Inspecting State in the Variables Panel

When paused, open the **Run and Debug** sidebar (`Ctrl+Shift+D`) and look at **Variables**.

**Current Cell scope:**

| Variable | Example value | Meaning |
|----------|--------------|---------|
| `pointer` | `3` | Current tape pointer (zero-indexed) |
| `value` | `65 (0x41) 'A'` | Current cell value: decimal, hex, printable char |

**Memory scope:**

Shows all tape cells from `[0]` to at least `[9]` (minimum 10 cells), extended to the last non-zero cell. Example:

```
[0]  0
[1]  72
[2]  101
[3]  108
...
```

**Stack Trace panel** shows the current instruction position (program counter as a line number in your source file).

---

## Test Runner

The test runner lets you define expected input/output pairs and verify your programs in a single command.

### Creating a `.bft` Test File

Create a file with a `.bft` extension. It uses TOML syntax:

```toml
[[test]]
name = "Hello World"
program = "hello_world.bf"
expected_output = "Hello World!\n"

[[test]]
name = "Echo input"
program = "cat.bf"
input = "Hello"
expected_output = "Hello"

[[test]]
name = "Null byte input"
program = "echo_null.bf"
input = "\u0000"
expected_output = "\u0000"
```

**Field reference:**

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `name` | Yes | — | Human-readable test name shown in output |
| `program` | Yes | — | Path to `.bf` file, **relative to the `.bft` file** |
| `input` | No | `""` | Bytes to feed to `,` (read) instructions |
| `expected_output` | Yes | — | Expected bytes written by `.` (output) instructions |

**String escapes in TOML:** `\n` (newline), `\t` (tab), `\\` (backslash), `\"` (quote), `\uXXXX` (4-digit Unicode, e.g. `\u0005` for byte value 5). The `\x` escape is **not** valid TOML.

**Trailing newline handling:** A single trailing `\n` is stripped from both `expected_output` and the actual output before comparison, so `"Hello\n"` and `"Hello"` are treated the same.

### Running Tests from VS Code

1. Open the Command Palette (`Ctrl+Shift+P`)
2. Run **Brainfuck: Run Test File**
3. A file picker opens — select your `.bft` file
4. Results appear in the **Brainfuck Tests** output channel

**Example output:**
```
Running: my_tests.bft
PASS  Hello World              (hello_world.bf)
PASS  Echo input               (cat.bf)
FAIL  Null byte input          (echo_null.bf)
      Expected : 0x00
      Actual   :
      Output ended early at byte 0
```

When any test fails, a notification appears:

> ⚠ 1 test(s) failed in my_tests.bft   **[Debug this failure]**

Click **Debug this failure** to automatically launch a `testDebug` session (see next section).

### Running Tests from the CLI

```bash
bf-testrunner my_tests.bft
```

**Output format:**
- `PASS  <name>  (<program.bf>)`
- `FAIL  <name>  (<program.bf>)` followed by expected/actual and diff location

**Exit codes:** `0` = all pass, `1` = any fail.

### Tips for Writing Tests

- Put `.bft` files in the same directory as your `.bf` files to keep `program` paths short.
- Use descriptive `name` values — they appear in debug session labels when debugging failures.
- One test case per scenario; don't combine unrelated programs in one `[[test]]`.
- Test edge cases separately: empty input, wrapping at 255/0, long output.

---

## testDebug Mode

`testDebug` is a special debug mode that runs your program against a specific expected output and pauses the debugger at the **exact byte** where the actual output diverges from expected.

### When to Use It

- After **"Debug this failure"** from the test runner (launched automatically)
- When you want to inspect program state at the precise moment a byte goes wrong
- For debugging output-correctness issues without manually stepping through every instruction

### How It Works

1. The program runs normally until it outputs a byte via `.`
2. That byte is compared against the next expected byte
3. If they match, execution continues
4. If they differ (or if the program ends early, or produces extra bytes), the debugger **pauses** and shows why

### Stop Reasons

| Stop reason | Meaning |
|-------------|---------|
| `testMismatch` | Output byte differed from expected at this position |
| `outputTooLong` | Program produced more bytes than expected |
| `outputTooShort` | Program ended before producing all expected bytes |

### The Test Mismatch Scope

When stopped, a third scope — **Test Mismatch** — appears in the Variables panel:

| Variable | Description |
|----------|-------------|
| `diverged_at_byte` | Zero-indexed byte position of the first difference |
| `expected_byte` | What was expected at that position |
| `actual_byte` | What the program actually produced (or `"(program ended early)"`) |
| `output_so_far` | All correct bytes output before the divergence |
| `expected_output` | The full expected output string |

### Manual Launch Configuration

You can also create a `testDebug` session manually:

```json
{
  "type": "brainfuck",
  "request": "testDebug",
  "name": "Debug failing test",
  "program": "${file}",
  "input": "",
  "expectedOutput": "Hello World!\n"
}
```

| Property | Description |
|----------|-------------|
| `program` | Path to the `.bf` file |
| `input` | Bytes fed to `,` instructions (same format as `.bft` input field) |
| `expectedOutput` | The exact output you expect the program to produce |

---

## AOT Precompilation

The precompiler compiles a `.bf` file to a standalone native executable using [Cranelift](https://cranelift.dev/) for code generation and your system's C linker.

**The pipeline:**
1. Parse `.bf` source → AST
2. Run the optimizer (constant folding + pattern recognition)
3. Generate Cranelift IR → native object file (`.o`)
4. Link with system `cc` / `link.exe` → standalone executable

The compiled binary has a 30,000-byte tape in BSS (zero-initialized), uses `putchar`/`getchar` for I/O, and includes a `main()` entry point — no runtime needed.

### Precompile from VS Code

1. Open the `.bf` file you want to compile
2. Open the Command Palette → **Brainfuck: Precompile to Executable**
3. A save dialog opens — choose where to save the output binary
4. A terminal opens showing compilation and link progress

### Precompile from the CLI

```bash
# Compile hello_world.bf to ./hello
bf-precompiler hello_world.bf -o hello

# Run it
./hello
```

If `-o` is omitted, the output binary is named after the input file without its extension.

### Requirements

- The `bf-precompiler` binary must be built with the `aot` feature:
  ```bash
  cargo build -p bf-precompiler --features aot --release
  ```
- A C compiler (`cc`) must be in `PATH` (macOS: Xcode Command Line Tools; Linux: `gcc` or `clang`; Windows: MSVC `link.exe`)

### Optimizer Patterns

The optimizer recognizes and replaces common BF patterns with more efficient IR:

| BF Pattern | Optimized form | Effect |
|------------|---------------|--------|
| `+++` | `Add(3)` | Constant folding of consecutive ops |
| `[-]` | `ZeroCell` | Clear current cell |
| `[->+<]` | `MoveRight(1)` | Move cell value right by 1 |
| `[-<+>]` | `MoveLeft(1)` | Move cell value left by 1 |
| `[->++<]` | `MultiplyRight(1, 2)` | Multiply current cell by 2 into cell+1 |
| `[>]` | `FindZeroRight` | Scan right to next zero cell |
| `[<]` | `FindZeroLeft` | Scan left to next zero cell |

---

## Configuration Reference

Open VS Code Settings (`Ctrl+,`) and search for `brainfuck` to see all options.

| Setting | Default | Description |
|---------|---------|-------------|
| `brainfuck.lspServerPath` | `""` | Absolute path to the `bf-lsp` binary. When empty, uses `target/debug/bf-lsp` relative to the workspace root. |
| `brainfuck.interpreterPath` | `""` | Absolute path to the `bf-interpreter` binary. |
| `brainfuck.dapServerPath` | `""` | Absolute path to the `bf-dap` binary. |
| `brainfuck.testRunnerPath` | `""` | Absolute path to the `bf-testrunner` binary. |
| `brainfuck.precompilerPath` | `""` | Absolute path to the `bf-precompiler` binary. |

**When to set paths:** Only set these if you've installed the binaries to a location outside the workspace (e.g., globally to `/usr/local/bin/`). If you're working from the source repo, leave them empty — the extension will automatically find the binaries in `target/debug/` after you run `cargo build --workspace`.

**Example settings.json:**
```json
{
  "brainfuck.interpreterPath": "/usr/local/bin/bf-interpreter",
  "brainfuck.lspServerPath": "/usr/local/bin/bf-lsp",
  "brainfuck.dapServerPath": "/usr/local/bin/bf-dap",
  "brainfuck.testRunnerPath": "/usr/local/bin/bf-testrunner",
  "brainfuck.precompilerPath": "/usr/local/bin/bf-precompiler"
}
```

---

## VS Code Commands Reference

Access all commands via the Command Palette (`Ctrl+Shift+P` / `Cmd+Shift+P`). Type "Brainfuck" to filter.

| Command | Description | Requires active `.bf` file |
|---------|-------------|--------------------------|
| `Brainfuck: Run Brainfuck Program` | Run via tree-walking interpreter | Yes |
| `Brainfuck: Run with JIT` | Run via Cranelift JIT compiler | Yes |
| `Brainfuck: Run with Auto JIT` | Run via adaptive JIT (hot loops only) | Yes |
| `Brainfuck: Format Document` | Format active file (same as `Shift+Alt+F`) | Yes |
| `Brainfuck: Run Test File` | Open file picker and run a `.bft` test suite | No |
| `Brainfuck: Precompile to Executable` | AOT-compile active file to native binary | Yes |

**Adding keybindings:** Open `keybindings.json` (`Ctrl+Shift+P` → "Open Keyboard Shortcuts (JSON)") and add:

```json
[
  { "key": "ctrl+shift+r", "command": "brainfuck.run",
    "when": "editorLangId == brainfuck" },
  { "key": "ctrl+shift+t", "command": "brainfuck.runTests" }
]
```

---

## Troubleshooting

### "Binary not found" or extension features don't work

**Symptom:** Running a program shows an error like `binary not found` or `spawn bf-interpreter ENOENT`.

**Cause:** The extension can't locate the Rust binary.

**Fix:**
1. Build from source: `cargo build --workspace` (in the repo root)
2. OR set the path explicitly in settings: `"brainfuck.interpreterPath": "/path/to/bf-interpreter"`

---

### Formatting does nothing or shows an error

**Symptom:** `Shift+Alt+F` has no effect.

**Causes and fixes:**
- The LSP server (`bf-lsp`) is not running — check the Output panel (select "Brainfuck LSP" from the dropdown) for errors
- The LSP binary is not found — set `brainfuck.lspServerPath` or build from source
- The file has a syntax error (unmatched brackets) — fix the errors first; the formatter returns the file unchanged if it can't parse it

---

### Debugger won't start (`F5` does nothing or shows error)

**Symptom:** Pressing `F5` opens a dialog saying the debug type is not recognized, or nothing happens.

**Causes and fixes:**
- No active `.bf` file — open a `.bf` file first, or add a `launch.json` with `"program": "${workspaceFolder}/my.bf"`
- The `bf-dap` binary is missing — build with `cargo build -p bf-dap` or set `brainfuck.dapServerPath`
- Verify the `.vscode/launch.json` has `"type": "brainfuck"` (not `"type": "bf"` or similar)

---

### Test runner shows "Error parsing .bft file"

**Symptom:** Running tests produces a parse error.

**Common causes:**
- TOML syntax error — check for missing quotes, invalid escape sequences, or mismatched `[[test]]` sections
- Using `\x05` escape — TOML does not support `\x` hex escapes; use `\u0005` instead
- Using `\u{5}` — TOML requires exactly 4 hex digits: `\u0005`, not `\u{5}`
- The `program` path doesn't exist — paths in `.bft` files are relative to the `.bft` file's directory

---

### Precompiler fails at the link step

**Symptom:** Compilation succeeds but linking fails with a linker error.

**Causes and fixes:**
- No C compiler in `PATH` — install Xcode Command Line Tools (Mac: `xcode-select --install`), `gcc`/`clang` (Linux), or MSVC (Windows)
- Wrong platform — the `bf-precompiler` binary must be built for the same OS you're running it on
- Missing `aot` feature — rebuild: `cargo build -p bf-precompiler --features aot --release`
