# Brainfuck Test Runner

The test runner lets you define expected input/output pairs for your BF programs and run them with a single command.

## `.bft` File Format

Test files use TOML syntax with one `[[test]]` section per test case.

```toml
[[test]]
name = "Hello World"
program = "hello_world.bf"
expected_output = "Hello World!\n"

[[test]]
name = "Echo uppercase A"
program = "cat.bf"
input = "A"
expected_output = "A"

[[test]]
name = "Binary escape in input"
program = "echo_byte.bf"
input = "\u0005"
expected_output = "\u0005"
```

### Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Human-readable test name, shown in output |
| `program` | string | Yes | Path to `.bf` file, relative to the `.bft` file |
| `input` | string | No | Bytes fed to `,` (read) instructions. Default: `""` |
| `expected_output` | string | Yes | Expected stdout. A trailing `\n` is stripped before comparison |

### String Escapes

TOML string escapes are supported: `\n`, `\t`, `\\`, `\"`, `\uXXXX` (4-digit hex, no braces). The `\x` hex escape is **not** supported by TOML — use `\uXXXX` with leading zeros instead (e.g., `\u0005` for byte value 5).

### Comparison

The test runner strips a single trailing `\n` from both the actual output and `expected_output` before comparing, so you don't need to account for a final newline in most cases. Comparison is byte-by-byte; the first differing position is reported on failure.

## CLI Usage

```bash
bf-testrunner my_tests.bft
```

**Output format:**
```
PASS  Hello World              (hello_world.bf)
FAIL  Echo uppercase A         (cat.bf)
      Expected : A
      Actual   : B
      First diff at byte 0: expected 'A' (0x41), got 'B' (0x42)
```

**Exit codes:** `0` = all tests passed · `1` = one or more tests failed

## VS Code Integration

Run **Brainfuck: Run Test File** from the Command Palette. A file picker opens for `*.bft` files. Results stream to the **Brainfuck Tests** output channel.

On failure, an information message appears with a **"Debug this failure"** button. See [DEBUGGING.md](DEBUGGING.md) for the testDebug flow.

## Tips

- Put `.bft` files next to your `.bf` files so relative `program` paths are short.
- Use `name` strings that describe the scenario, not just the file name — they appear in the output and in debug session labels.
- Keep one test per meaningful input/output combination; don't combine unrelated cases.
