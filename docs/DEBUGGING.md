# Brainfuck Debugger

The extension integrates with VS Code's standard debug UI via the Debug Adapter Protocol (DAP).

## Standard Debug Mode

### Setup

Press `F5` with a `.bf` file open. VS Code will use the default launch configuration. Or add a custom one to `.vscode/launch.json`:

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

### Breakpoints

Click the gutter to the left of a line number to set a breakpoint. The debugger resolves breakpoints to the first BF instruction on that line. Lines with only comments or whitespace are marked *unverified*.

### Stepping

| Action | Keybinding | Effect |
|--------|-----------|--------|
| Continue | `F5` | Run to next breakpoint or end |
| Next (Step Over) | `F10` | Execute one BF instruction |
| Step Out | `Shift+F11` | Run to the end of the current loop body |

### Variables Panel

When stopped, two scopes are shown:

**Current Cell**
- `pointer` — the current tape pointer (zero-indexed)
- `value` — the cell value as `decimal (0xHH) 'char'`

**Memory**
- `[0]` through `[N]` — all cells from 0 to the last non-zero cell, minimum 10

## testDebug Mode

`testDebug` is a special debug mode that compares the program's output against an expected byte sequence as the program runs. The debugger pauses the moment output diverges from expected, letting you inspect the interpreter state at the exact point of failure.

### Launch Configuration

```json
{
  "type": "brainfuck",
  "request": "testDebug",
  "name": "Test Debug",
  "program": "${file}",
  "input": "",
  "expectedOutput": "Hello World!\n"
}
```

| Property | Description |
|----------|-------------|
| `program` | Path to the `.bf` file |
| `input` | Bytes to feed to `,` (read) instructions |
| `expectedOutput` | Expected stdout bytes |

### Stop Reasons

| Reason | Meaning |
|--------|---------|
| `testMismatch` | Program output a byte that differed from expected |
| `outputTooLong` | Program output more bytes than expected |
| `outputTooShort` | Program ended before producing all expected bytes |

### Test Mismatch Scope

When the debugger stops with a test-related reason, a third scope — **Test Mismatch** — appears in the Variables panel with:

| Variable | Description |
|----------|-------------|
| `diverged_at_byte` | Zero-indexed byte position of the first mismatch |
| `expected_byte` | What was expected at that position |
| `actual_byte` | What the program actually produced |
| `output_so_far` | All correctly matching bytes produced before the divergence |
| `expected_output` | The full expected output |

### "Debug this failure" Flow

When **Brainfuck: Run Test File** finds a failing test, an information message appears with a **"Debug this failure"** button. Clicking it automatically launches a `testDebug` session with `program`, `input`, and `expectedOutput` pre-filled from the failing test case.
