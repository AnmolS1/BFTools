# Technology Reference

A comprehensive overview of every tool, library, protocol, and technology used in BFTools.

---

## Languages

### Rust (backend — 7 crates)

All interpreter, compiler, LSP, DAP, and test runner logic is written in Rust. The workspace uses **Rust edition 2021** and targets **rustc 1.82.0** as the minimum supported compiler.

Rust was chosen for:
- Memory safety without a garbage collector (critical for a JIT compiler that manages raw pointers)
- Zero-cost abstractions (the tree-walking interpreter and JIT share the same bytecode representation)
- Cargo's workspace feature (7 crates share a single `Cargo.lock` and build cache)
- Excellent cross-compilation support (the CI builds for Linux, macOS, and Windows from a single workflow)

### TypeScript (VS Code extension)

The VS Code extension (`src/`) is written in **TypeScript 5.3**, compiled to CommonJS ES2020 via `tsc`. TypeScript was chosen because VS Code's extension API is first-class TypeScript, and the `vscode-languageclient` library provides typed wrappers for LSP communication.

---

## Rust Crates

### Workspace Layout

```
Cargo.toml          # workspace root
crates/
  bf-core/          # foundation: lexer, parser, AST, validator, .bft parser
  bf-formatter/     # 7-rule canonical formatter
  bf-lsp/           # Language Server Protocol server
  bf-interpreter/   # interpreter + Cranelift JIT + auto-JIT
  bf-precompiler/   # optimizer IR + Cranelift AOT compiler
  bf-dap/           # Debug Adapter Protocol server
  bf-testrunner/    # .bft test runner
```

### Core Dependencies

| Crate | Version | Used by | Purpose |
|-------|---------|---------|---------|
| `serde` | 1.x | bf-core, bf-dap | Derive-based serialization/deserialization |
| `serde_json` | 1.x | bf-dap | JSON parsing and emission for DAP messages |
| `toml` | 0.8 | bf-core | Parse `.bft` TOML test files |
| `tower-lsp` | 0.20 | bf-lsp | Async LSP server framework |
| `tokio` | 1.x | bf-lsp | Async runtime for the LSP server |
| `dashmap` | 5.x | bf-lsp | Concurrent hash map for open document storage |
| `cranelift-codegen` | 0.113 | bf-interpreter, bf-precompiler | Cranelift code generation backend |
| `cranelift-frontend` | 0.113 | bf-interpreter, bf-precompiler | Function builder API for emitting IR |
| `cranelift-module` | 0.113 | bf-interpreter, bf-precompiler | Module abstraction (shared by JIT and AOT) |
| `cranelift-jit` | 0.113 | bf-interpreter | JIT module: allocates executable memory |
| `cranelift-object` | 0.113 | bf-precompiler | AOT module: emits ELF/Mach-O/COFF object files |
| `target-lexicon` | 0.12 | bf-precompiler | Parse and represent target triple strings |

### Why Cranelift 0.113

Cranelift was chosen as the code generation backend for both JIT and AOT compilation because:
- It is pure Rust (no C++ dependencies, unlike LLVM)
- It provides a clean IR builder API (`FunctionBuilder`, `InstBuilder`)
- The `cranelift-jit` and `cranelift-object` crates share the same IR, so the optimizer output feeds both JIT and AOT paths unchanged
- Version 0.113 was the stable release available for rustc 1.82.0

---

## Node.js / TypeScript Dependencies

| Package | Version | Purpose |
|---------|---------|---------|
| `vscode-languageclient` | 9.x | Connects VS Code to the `bf-lsp` server over stdio (LSP client) |
| `@vscode/vsce` | 3.x | Packages and publishes the `.vsix` extension bundle |
| `typescript` | 5.3 | TypeScript compiler |
| `@types/vscode` | 1.85 | VS Code extension API type definitions |
| `@types/node` | 25.x | Node.js type definitions |

---

## Protocols

### Language Server Protocol (LSP)

[LSP](https://microsoft.github.io/language-server-protocol/) is a JSON-RPC protocol that separates editor-agnostic language intelligence from the editor itself. BFTools implements an LSP server (`bf-lsp`) that provides:

- **Text synchronization** (`textDocument/didOpen`, `didChange`, `didClose`) — the server maintains an in-memory copy of every open `.bf` file
- **Formatting** (`textDocument/formatting`) — calls the `bf-formatter` library and returns a list of `TextEdit` operations
- **Diagnostics** (`textDocument/publishDiagnostics`) — calls the `bf-core` validator and pushes `Diagnostic` objects to the client

The server runs as a child process (`bf-lsp`) communicating over stdio. The VS Code extension uses `vscode-languageclient` to start the process and translate LSP messages into VS Code API calls.

**Transport:** JSON-RPC 2.0 over stdio, framed with `Content-Length: N\r\n\r\n` headers.

### Debug Adapter Protocol (DAP)

[DAP](https://microsoft.github.io/debug-adapter-protocol/) is a JSON protocol that separates debugger implementation from the editor UI. BFTools implements a DAP server (`bf-dap`) that handles:

| DAP Request | What it does |
|------------|-------------|
| `initialize` | Returns capabilities (supportsConfigurationDoneRequest, supportsStepOut) |
| `launch` | Parses `.bf` source, compiles to bytecode, sets up the interpreter; supports `testDebug` mode |
| `configurationDone` | Signals that breakpoints have been sent; starts execution |
| `setBreakpoints` | Resolves breakpoints from source lines to bytecode instruction indices |
| `threads` | Returns a single thread ("Main Thread") |
| `stackTrace` | Returns the current PC as a stack frame with source location |
| `scopes` | Returns Current Cell, Memory, and (in testDebug mode) Test Mismatch scopes |
| `variables` | Returns variable values for a given scope reference |
| `continue` | Runs until breakpoint, test mismatch, or program end |
| `next` / `stepIn` | Steps one BF instruction |
| `stepOut` | Runs until the current loop nesting level decreases |
| `terminate` / `disconnect` | Cleans up and exits |

**Transport:** JSON over stdio, framed with `Content-Length: N\r\n\r\n` headers (same framing as LSP).

---

## VS Code Extension APIs

### TextMate Grammar

`syntaxes/brainfuck.tmLanguage.json` is a TextMate grammar in JSON format. It defines token patterns using regular expressions that VS Code uses for syntax highlighting. The grammar scope is `source.brainfuck`.

**Token scopes defined:**
- `comment.line.double-slash.brainfuck` — `//` line comments
- `comment.block.brainfuck` — `/* */` block comments
- `keyword.operator.brainfuck` — BF operators (`+`, `-`, `<`, `>`, `.`, `,`, `[`, `]`)

### Language Configuration

`language-configuration.json` provides editor behavior for the `brainfuck` language:
- **Comment toggling:** `Ctrl+/` adds/removes `//` comments
- **Block comments:** `Shift+Alt+A` adds/removes `/* */` comments
- **Bracket matching:** `[` and `]` are highlighted as matching pairs
- **Auto-closing:** typing `[` inserts `]` automatically

### Debug Adapter Factory

`src/dap-client.ts` registers a `DebugAdapterDescriptorFactory` that tells VS Code how to launch `bf-dap`. The factory uses `DebugAdapterExecutable` to spawn the process — VS Code then communicates with it via stdin/stdout.

### Configuration Properties

Five `brainfuck.*` settings are declared in `package.json` under `contributes.configuration`. VS Code generates the settings UI and schema validation automatically from these declarations.

### Commands

Six commands are declared in `package.json` under `contributes.commands` and registered in `src/extension.ts` via `vscode.commands.registerCommand()`.

---

## Interpreter Architecture

### Bytecode Compilation

The `bf_core` AST is compiled to a flat array of `Instruction` structs (bytecode) by `bf_interpreter::compile()`. Each instruction has an `Opcode` and an optional operand:

| Opcode | Operand | Meaning |
|--------|---------|---------|
| `Add` | u8 delta | `current_cell += delta` |
| `Sub` | u8 delta | `current_cell -= delta` |
| `Right` | usize count | `pointer += count` |
| `Left` | usize count | `pointer -= count` |
| `Input` | — | Read one byte from stdin |
| `Output` | — | Write current cell to stdout |
| `JumpIfZero` | usize target | If cell == 0, jump to target |
| `JumpIfNotZero` | usize target | If cell != 0, jump to target |

### Tree-Walking Interpreter

`Interpreter` maintains a `Memory` (30,000-byte tape), a `pointer`, and a `pc` (program counter). It steps through the bytecode array one instruction at a time. The `step()` method returns a `StepResult` enum (`Ok`, `Output(byte)`, `Input`, `Finished`, `RuntimeError`).

### Cranelift JIT

`JitCompiler` takes the bytecode array and emits Cranelift IR for a single function. The function signature is:
```
fn(tape: *mut u8, tape_len: usize, ctx: *mut JitIoContext) -> ()
```

I/O is performed via callback function pointers in `JitIoContext` (write_byte, read_byte), allowing the caller to control stdin/stdout without linking a C runtime.

### Auto-JIT (Adaptive)

`AutoJitInterpreter` wraps the tree-walking interpreter with a loop hit counter. When any `JumpIfNotZero` instruction is executed for the 3rd time, the surrounding loop is JIT-compiled and patched into the execution flow. Subsequent iterations run natively.

---

## AOT Compilation Pipeline

```
.bf source
    │
    ▼
bf_core::lexer::tokenize()
    │
    ▼
bf_core::parser::parse()         → AST (Vec<Node>)
    │
    ▼
bf_precompiler::optimizer::optimize()  → Vec<OptimizedOp>
    │
    ▼
bf_precompiler::compiler::compile_to_object()
    │   (Cranelift ObjectModule, emits ELF/Mach-O/COFF)
    ▼
  hello.o
    │
    ▼
bf_precompiler::compiler::link_executable()
    │   (calls system `cc` or `link.exe`)
    ▼
  hello   (standalone native executable)
```

### Optimizer IR (`OptimizedOp`)

| Variant | Description |
|---------|-------------|
| `Add(u8)` | Folded consecutive increments |
| `Sub(u8)` | Folded consecutive decrements |
| `Right(usize)` | Folded consecutive right moves |
| `Left(usize)` | Folded consecutive left moves |
| `Read` | Input instruction |
| `Write` | Output instruction |
| `Loop(Vec<OptimizedOp>)` | Unrecognized loop body |
| `ZeroCell` | `[-]` — clears current cell |
| `MoveRight(usize)` | `[->+<]` — moves cell value right |
| `MoveLeft(usize)` | `[-<+>]` — moves cell value left |
| `MultiplyRight(usize, u8)` | `[->N+<]` — multiply into adjacent cell |
| `FindZeroRight` | `[>]` — scan right to next zero |
| `FindZeroLeft` | `[<]` — scan left to next zero |

### Object File Details

- The AOT module uses `cranelift_object::ObjectModule` with position-independent code (PIC) on non-Windows targets
- The tape is declared as a 30,000-byte BSS symbol (`bf_tape`)
- `putchar` and `getchar` are declared as external imports
- A `main` function is emitted that calls into the compiled BF logic and returns 0

---

## Testing

### Framework

All tests use Rust's built-in `#[test]` framework (no external test crate). Tests are declared with `#[test]` attributes inside `#[cfg(test)]` modules.

### Test Organization

| Location | Type | Count |
|----------|------|-------|
| `crates/bf-core/src/*.rs` (inline) | Unit tests | 29 |
| `crates/bf-dap/src/*.rs` (inline) | Unit tests | 11 |
| `crates/bf-formatter/src/*.rs` (inline) | Unit tests | 10 |
| `crates/bf-lsp/src/*.rs` (inline) | Unit tests | 8 |
| `crates/bf-interpreter/src/*.rs` (inline) | Unit tests | 8 |
| `crates/bf-precompiler/src/*.rs` (inline) | Unit tests | 10 |
| `crates/bf-testrunner/src/*.rs` (inline) | Unit tests | 3 |
| `crates/bf-formatter/tests/integration.rs` | Integration | 4 |
| `crates/bf-interpreter/tests/integration.rs` | Integration | 8 (1 ignored) |
| `crates/bf-dap/tests/integration.rs` | Integration | 6 |
| `crates/bf-testrunner/tests/integration.rs` | Integration | 5 |
| **Total** | | **101 pass, 1 ignored** |

### Integration Test Pattern

Integration tests in `crates/*/tests/` use `env!("CARGO_MANIFEST_DIR")` to build absolute paths to test fixtures:

```rust
fn fixture_path(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../../test-fixtures");
    p.push(name);
    p
}
```

This ensures tests work regardless of the working directory when `cargo test` is invoked.

---

## CI/CD

### GitHub Actions Workflows

**`ci.yml`** — runs on every push to `main`/`prod` and every pull request to `main`:
1. Matrix build: ubuntu-latest, macos-latest, windows-latest
2. Rust 1.82.0 via `dtolnay/rust-toolchain`
3. `cargo build --workspace --release`
4. `cargo test --workspace`
5. Node 20, `npm ci`, `npm run compile`
6. Upload platform-specific binaries as artifacts

**`release.yml`** — runs on every `v*` tag push:
1. Same matrix build as CI
2. Package extension: `npx vsce package --no-dependencies`
3. Create GitHub Release with all binaries and `.vsix` attached

### Dependency Caching

`actions/cache@v4` caches `~/.cargo/registry`, `~/.cargo/git`, and the `target/` directory, keyed by `Cargo.lock` hash. This dramatically reduces CI time after the first run.

---

## Packaging

### VS Code Extension (`.vsix`)

The extension is packaged with `@vscode/vsce`. The `.vscodeignore` file excludes everything that's not needed at runtime:
- Source files (`src/`, `*.rs`, `crates/`)
- Build artifacts (`target/`, `out/` is included)
- Development files (`tsconfig.json`, `Cargo.toml`, `tasks.md`)
- CI configuration (`.github/`)

The packaged extension contains:
- `out/` — compiled TypeScript (JavaScript)
- `syntaxes/` — TextMate grammar
- `language-configuration.json`
- `package.json`
- `README.md`

The Rust binaries are **not** bundled in the `.vsix`. Users must build them from source or download them from the GitHub Release.

### Rust Binaries

Five standalone binaries are produced by `cargo build --workspace --release`:

| Binary | Crate | Description |
|--------|-------|-------------|
| `bf-lsp` | bf-lsp | LSP server (stdio) |
| `bf-dap` | bf-dap | DAP debug server (stdio) |
| `bf-interpreter` | bf-interpreter | CLI interpreter with JIT modes |
| `bf-testrunner` | bf-testrunner | CLI test runner for `.bft` files |
| `bf-precompiler` | bf-precompiler | CLI optimizer + AOT compiler |
