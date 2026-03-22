# BFTools — Brainfuck Language Tools for VS Code

Full-featured Brainfuck development environment: syntax highlighting, auto-formatting, lint diagnostics, interactive debugging, JIT execution, test runner, and native AOT compilation.

## Features

| Feature | Description |
|---------|-------------|
| **Syntax highlighting** | Token-level colorization for BF operators and comments |
| **Formatting** | 7-rule canonical formatter via LSP (`Shift+Alt+F`) |
| **Lint diagnostics** | Six built-in rules: invalid chars, unmatched brackets, empty loops, redundant ops, dead code |
| **Debugging** | Full DAP integration: breakpoints, step, step-over loop, inspect memory/cell |
| **JIT execution** | Cranelift-powered JIT (`--jit`) and adaptive hot-path auto-JIT (`--auto-jit`) |
| **Test runner** | `.bft` TOML test files; PASS/FAIL output; "Debug this failure" one-click |
| **Precompiler** | AOT compilation to native executable via Cranelift + system linker |

## Installation

### From VS Code Marketplace

Search for **"Brainfuck"** by *Ponderance* in the Extensions panel, or:

```
ext install Ponderance.brainfuck-vscode
```

### Build from Source

Requirements: **Rust ≥ 1.82**, **Node ≥ 18**, **npm**

```bash
git clone https://github.com/AnmolS1/BFTools
cd BFTools

# Build all Rust binaries
cargo build --workspace --release

# Build the extension
npm install
npm run compile

# Package and install locally
npx vsce package
code --install-extension brainfuck-vscode-*.vsix
```

## Usage

### Syntax Highlighting

Open any `.bf` file — colorization is applied automatically. BF operators (`+`, `-`, `<`, `>`, `.`, `,`, `[`, `]`) are highlighted as keyword/operator tokens. `//` line comments and `/* */` block comments receive comment styling.

### Formatting

Open any `.bf` file and press `Shift+Alt+F`, or run **Brainfuck: Format Document** from the Command Palette. Enable `editor.formatOnSave` to format automatically on every save.

The formatter applies 7 canonical rules: instruction grouping, inline loops (≤ 80 chars), block loop indentation (tabs), comment placement, and a single trailing newline. Formatting is idempotent. See [docs/FORMATTING.md](docs/FORMATTING.md) for the complete rule reference.

### Linting

Diagnostics appear automatically in the Problems panel as you type.

| Rule | Name | Description | Severity |
|------|------|-------------|----------|
| `BF001` | Invalid character | Non-BF, non-comment character in source | Error |
| `BF002` | Unmatched `[` | Opening bracket with no matching close | Error |
| `BF003` | Unmatched `]` | Closing bracket with no matching open | Error |
| `BF004` | Empty loop | `[]` — loop that can never do anything | Warning |
| `BF005` | Redundant ops | `+-` or `-+` adjacent sequences that cancel | Hint |
| `BF006` | Dead code | Instructions following a `[-]` (cell is already zero) | Warning |

### Running Programs

Open a `.bf` file and use the Command Palette:

| Command | Mode | Description |
|---------|------|-------------|
| **Brainfuck: Run Brainfuck Program** | Interpreter | Tree-walking bytecode interpreter |
| **Brainfuck: Run with JIT** | JIT | Cranelift JIT — compiles full program to native code |
| **Brainfuck: Run with Auto JIT** | Auto-JIT | Interprets until a hot loop, then JITs that loop |

Each command opens a VS Code terminal. Standard input/output is connected to the terminal.

CLI equivalents:
```bash
bf-interpreter hello_world.bf
bf-interpreter --jit hello_world.bf
bf-interpreter --auto-jit hello_world.bf
bf-interpreter --no-jit hello_world.bf   # force interpreter even if jit feature built
```

> **Note:** JIT and auto-JIT require building `bf-interpreter` with `--features jit`. The default release binary includes this feature.

### Debugging

Press `F5` with a `.bf` file open. VS Code will use the default launch configuration. To customize:

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

**Breakpoints:** Click the gutter next to a line number. Breakpoints resolve to the first BF instruction on that line. Lines with only comments are marked unverified.

**Stepping:**

| Action | Keybinding | Effect |
|--------|-----------|--------|
| Continue | `F5` | Run to next breakpoint or end |
| Next (Step Over) | `F10` | Execute one BF instruction |
| Step Out | `Shift+F11` | Run to end of the current loop body |

**Variables panel** shows two scopes when stopped:
- *Current Cell* — `pointer` (tape index) and `value` (decimal, hex, printable char)
- *Memory* — cells `[0]` through `[N]` where N is the last non-zero cell (minimum 10)

See [docs/DEBUGGING.md](docs/DEBUGGING.md) for testDebug mode (debugging a specific test case failure).

### Testing

Create a `.bft` TOML test file:

```toml
[[test]]
name = "Hello World"
program = "hello_world.bf"
expected_output = "Hello World!\n"

[[test]]
name = "Echo A"
program = "cat.bf"
input = "A"
expected_output = "A"
```

Run **Brainfuck: Run Test File** from the Command Palette. A file picker opens for `*.bft` files. Results stream to the **Brainfuck Tests** output channel:

```
PASS  Hello World              (hello_world.bf)
FAIL  Echo A                   (cat.bf)
      Expected : A
      Actual   : B
      First diff at byte 0: expected 'A' (0x41), got 'B' (0x42)
```

When a test fails, click **"Debug this failure"** to launch a `testDebug` session that pauses the program at the exact byte where output diverges. See [docs/TESTING.md](docs/TESTING.md) for the full `.bft` format reference.

CLI usage:
```bash
bf-testrunner tests.bft
# exit 0 = all pass, 1 = any fail
```

### Precompiling

Run **Brainfuck: Precompile to Executable** from the Command Palette. A save dialog opens — choose the output file name. The extension compiles via Cranelift AOT and links with the system C compiler (`cc` on macOS/Linux, `link.exe` on Windows).

CLI usage:
```bash
bf-precompiler --features aot hello_world.bf -o hello
./hello
```

> **Requirements:** AOT compilation requires building `bf-precompiler` with `--features aot` and a system C compiler in `PATH`. The default release binary includes this feature.

## VS Code Commands

| Command | Description |
|---------|-------------|
| `Brainfuck: Run Brainfuck Program` | Run active `.bf` file via interpreter |
| `Brainfuck: Run with JIT` | Run active `.bf` file via Cranelift JIT |
| `Brainfuck: Run with Auto JIT` | Run active `.bf` file via adaptive JIT |
| `Brainfuck: Format Document` | Format active `.bf` file (same as `Shift+Alt+F`) |
| `Brainfuck: Run Test File` | Open file picker and run a `.bft` test suite |
| `Brainfuck: Precompile to Executable` | AOT-compile active `.bf` file to native binary |

## Configuration

All settings are under `brainfuck.*` in VS Code settings (`Ctrl+,`):

| Setting | Default | Description |
|---------|---------|-------------|
| `brainfuck.lspServerPath` | `""` | Path to `bf-lsp` binary. Empty = use `target/debug/bf-lsp` |
| `brainfuck.interpreterPath` | `""` | Path to `bf-interpreter` binary |
| `brainfuck.dapServerPath` | `""` | Path to `bf-dap` binary |
| `brainfuck.testRunnerPath` | `""` | Path to `bf-testrunner` binary |
| `brainfuck.precompilerPath` | `""` | Path to `bf-precompiler` binary |

When a path is empty, the extension looks for the binary in `target/debug/` relative to the workspace root — convenient when working from source.

## `.bft` Test Format Quick Reference

```toml
[[test]]
name = "Human-readable name"          # required
program = "relative/path.bf"          # required; relative to the .bft file
input = "optional stdin bytes"        # optional; default ""
expected_output = "expected stdout"   # required; trailing \n stripped before compare
```

TOML string escapes: `\n`, `\t`, `\\`, `\"`, `\uXXXX` (4-digit hex, e.g. `\u0005` for byte 5). The `\x` escape is not valid in TOML.

## Architecture

```
crates/
  bf-core/         Lexer, parser, AST, validator, .bft test file parser (serde + toml)
  bf-formatter/    7-rule canonical formatter (lib + CLI)
  bf-lsp/          Language server: formatting + diagnostics (tower-lsp, tokio)
  bf-interpreter/  Tree-walking interpreter + Cranelift JIT + adaptive auto-JIT
  bf-precompiler/  Optimizer IR (constant folding, pattern recognition) + Cranelift AOT
  bf-dap/          Debug Adapter Protocol server (DAP over stdio, serde_json)
  bf-testrunner/   .bft test runner (lib + CLI)
src/               VS Code extension (TypeScript, vscode-languageclient)
syntaxes/          TextMate grammar (.tmLanguage.json)
test-fixtures/     Sample .bf and .bft files
docs/              Detailed reference documentation
```

## Documentation

- [docs/USER_GUIDE.md](docs/USER_GUIDE.md) — Complete user guide for all extension features
- [docs/FORMATTING.md](docs/FORMATTING.md) — All 7 formatting rules with examples
- [docs/DEBUGGING.md](docs/DEBUGGING.md) — Debugger setup, stepping, testDebug mode
- [docs/TESTING.md](docs/TESTING.md) — `.bft` format reference, test runner CLI
- [docs/TECHNOLOGY.md](docs/TECHNOLOGY.md) — Tools and technologies used

## Contributing

1. Fork and clone: `git clone https://github.com/AnmolS1/BFTools`
2. Build: `cargo build --workspace && npm install && npm run compile`
3. Test: `cargo test --workspace` — all 101 tests must pass
4. Submit a pull request against `main`

## License

MIT
