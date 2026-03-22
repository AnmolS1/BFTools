# Brainfuck Formatter — Rules Reference

The formatter produces a canonical representation with consistent indentation, grouping, and comment placement. Formatting is idempotent: running it twice produces the same output.

## Rule 1 — Instruction grouping

Consecutive `+`, `-`, `<`, `>` instructions on the same cell are grouped onto one line, separated by a single space from adjacent instruction groups.

```bf
// Input
+ + + - > > <

// Formatted
+++ -- >><
```

## Rule 2 — Loop on a single line (inline loop)

A loop whose body fits within the 80-character line limit is kept on one line:

```bf
[-]        // Zero-cell idiom — stays inline
[->+<]     // Simple move — stays inline
```

## Rule 3 — Loop on multiple lines (block loop)

When a loop body would exceed 80 characters, it is broken into multiple lines. The opening `[` stays on the preceding line (or its own line at column 0 for top-level), the body is indented by one tab per nesting level, and `]` is on its own line at the parent indentation level.

```bf
[
    >>>++++[>++++++++<-]>
    <<<<-
]
```

## Rule 4 — Indentation

Each loop nesting level adds one tab character (`\t`). Spaces are never used for indentation.

## Rule 5 — Inline comments

A `//`-style comment that follows BF instructions on the same line stays on the same line, separated by two spaces:

```bf
>++  // move right and set to 2
```

## Rule 6 — Block comments

A standalone comment line (no BF instructions before it on that line) is preserved as its own line at the current indentation level:

```bf
[
    // This comment is inside the loop
    -
]
```

## Rule 7 — Trailing newline

The formatted output always ends with exactly one newline character (`\n`). No blank lines are inserted between groups or loops.

---

## Configuration

No formatter options are currently exposed — the canonical style is fixed. This ensures all BF files in a project look identical regardless of who wrote them.
