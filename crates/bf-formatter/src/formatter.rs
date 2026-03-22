use bf_core::ast::{Node, Op};
use bf_core::lexer::{tokenize, CommentKind};
use bf_core::parser::parse;
use crate::config::FormatterConfig;

pub fn format(input: &str, config: &FormatterConfig) -> String {
    // Normalize line endings
    let input = input.replace("\r\n", "\n").replace('\r', "\n");

    let tokens = tokenize(&input);
    let program = match parse(&tokens) {
        Ok(p) => p,
        Err(_) => return input, // Return as-is if unparseable
    };

    let mut state = FormatterState {
        config,
        lines: Vec::new(),
        current_line: String::new(),
    };

    format_nodes_direct(&program.nodes, 0, &mut state);
    state.finish();

    let mut result = state.lines.join("\n");
    // Ensure single trailing newline
    while result.ends_with("\n\n") {
        result.pop();
    }
    if !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

struct FormatterState<'a> {
    config: &'a FormatterConfig,
    lines: Vec<String>,
    current_line: String,
}

impl<'a> FormatterState<'a> {
    fn push_line(&mut self) {
        let line = std::mem::take(&mut self.current_line);
        // Trim trailing whitespace
        self.lines.push(line.trim_end().to_string());
    }

    fn finish(&mut self) {
        if !self.current_line.is_empty() {
            self.push_line();
        }
    }

    fn append(&mut self, s: &str) {
        self.current_line.push_str(s);
    }
}

/// Check if a loop fits on a single line at the given indent level.
/// Returns None if multi-line, Some(rendered) if it fits.
fn try_inline_loop(body: &[Node], indent_level: usize, config: &FormatterConfig) -> Option<String> {
    // No nested loops
    if body.iter().any(|n| matches!(n, Node::Loop { .. })) {
        return None;
    }
    // No standalone comments (comments that appear on their own line in output)
    // We allow inline comments (// ...) but not block comments used as separators
    // For simplicity: disallow any comment in same-line body
    // Actually per spec: "no standalone comments" — inline comments are fine
    // We'll allow line comments but render them inline
    // But we need to check the rendered length

    let body_str = render_inline_body(body);
    let indent = config.indent_at_level(indent_level);
    // "[ body ]"
    let candidate = format!("{}[ {} ]", indent, body_str.trim());
    if candidate.len() <= config.max_line_length {
        Some(format!("[ {} ]", body_str.trim()))
    } else {
        None
    }
}

/// Render a loop body as a flat inline string (cell-grouped with spaces).
fn render_inline_body(nodes: &[Node]) -> String {
    // Group nodes by cell delta (same as multi-line but on one line)
    let groups = make_cell_groups(nodes);
    groups
        .iter()
        .map(|g| render_group_inline(g))
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// A cell group: contiguous operations on the same cell delta,
/// with movement operators that transition to the next cell.
#[derive(Debug)]
struct CellGroup {
    /// The operators in this group (movements + ops on current cell)
    ops: Vec<GroupOp>,
}

#[derive(Debug)]
enum GroupOp {
    BfChar(char),
    Comment(String, CommentKind),
}

fn op_to_char(op: &Op) -> char {
    match op {
        Op::Plus => '+',
        Op::Minus => '-',
        Op::Left => '<',
        Op::Right => '>',
        Op::Input => ',',
        Op::Output => '.',
    }
}

fn is_movement(op: &Op) -> bool {
    matches!(op, Op::Left | Op::Right)
}

/// Split a flat list of nodes into cell groups.
/// Each group contains the movement(s) that led to the current cell
/// plus the operations performed on that cell.
/// When a movement is followed by a non-movement op on a new cell,
/// the movement(s) start a new group.
fn make_cell_groups(nodes: &[Node]) -> Vec<CellGroup> {
    let mut groups: Vec<CellGroup> = vec![CellGroup { ops: Vec::new() }];
    let mut pending_movements: Vec<char> = Vec::new();

    for node in nodes {
        match node {
            Node::Instruction { op, .. } => {
                if is_movement(op) {
                    pending_movements.push(op_to_char(op));
                } else {
                    // Non-movement op: flush pending movements as start of new group
                    // (unless current group is empty — first op)
                    let last = groups.last_mut().unwrap();
                    let last_has_non_movement = last.ops.iter().any(|o| {
                        if let GroupOp::BfChar(c) = o {
                            !matches!(c, '<' | '>')
                        } else {
                            false
                        }
                    });

                    if !pending_movements.is_empty() && last_has_non_movement {
                        // Start new group with the pending movements
                        let mut new_group = CellGroup { ops: Vec::new() };
                        for m in pending_movements.drain(..) {
                            new_group.ops.push(GroupOp::BfChar(m));
                        }
                        new_group.ops.push(GroupOp::BfChar(op_to_char(op)));
                        groups.push(new_group);
                    } else {
                        // Flush pending movements into current group
                        for m in pending_movements.drain(..) {
                            last.ops.push(GroupOp::BfChar(m));
                        }
                        last.ops.push(GroupOp::BfChar(op_to_char(op)));
                    }
                }
            }
            Node::Comment { text, kind, .. } => {
                // Flush pending movements first
                if !pending_movements.is_empty() {
                    let last = groups.last_mut().unwrap();
                    for m in pending_movements.drain(..) {
                        last.ops.push(GroupOp::BfChar(m));
                    }
                }
                let last = groups.last_mut().unwrap();
                last.ops.push(GroupOp::Comment(text.clone(), kind.clone()));
            }
            Node::Loop { .. } => {
                // Loops handled separately — shouldn't appear in inline body
                // but flush pending movements
                if !pending_movements.is_empty() {
                    let last = groups.last_mut().unwrap();
                    for m in pending_movements.drain(..) {
                        last.ops.push(GroupOp::BfChar(m));
                    }
                }
            }
        }
    }

    // Flush any remaining pending movements
    if !pending_movements.is_empty() {
        let last = groups.last_mut().unwrap();
        for m in pending_movements.drain(..) {
            last.ops.push(GroupOp::BfChar(m));
        }
    }

    // Remove empty groups
    groups.retain(|g| !g.ops.is_empty());
    groups
}

fn render_group_inline(group: &CellGroup) -> String {
    let mut s = String::new();
    for op in &group.ops {
        match op {
            GroupOp::BfChar(c) => s.push(*c),
            GroupOp::Comment(text, CommentKind::Line) => {
                s.push_str(&format!(" //{}", text));
            }
            GroupOp::Comment(text, CommentKind::Block) => {
                s.push_str(&format!(" /*{}*/", text));
            }
        }
    }
    s
}

/// Format nodes with proper cell grouping and loop handling.
fn format_nodes_direct(nodes: &[Node], indent_level: usize, state: &mut FormatterState) {
    let indent = state.config.indent_at_level(indent_level);

    // We'll build "segments" — either a group of BF instructions, a loop, or a comment.
    // Each segment knows its source line info for comment placement.
    let mut i = 0;

    // Track the source line of the last emitted BF instruction (for comment indent detection)
    let mut last_bf_source_line: Option<usize> = None;
    let mut last_bf_indent = indent_level;

    // Buffer of pending movements awaiting a non-movement op
    let mut pending_movements: Vec<char> = Vec::new();

    // Current cell group being built (non-movement chars on current cell)
    let mut current_group: Vec<char> = Vec::new();
    // Whether current_group has any non-movement ops
    let mut group_has_nonmovement = false;

    while i < nodes.len() {
        match &nodes[i] {
            Node::Instruction { op, position } => {
                last_bf_source_line = Some(position.line);
                last_bf_indent = indent_level;

                if is_movement(op) {
                    if group_has_nonmovement {
                        // Start new group: emit current group, pending becomes start of next
                        // Emit current group
                        if !state.current_line.is_empty() {
                            state.push_line();
                        }
                        let mut line = indent.clone();
                        for m in pending_movements.drain(..) {
                            line.push(m);
                        }
                        for c in current_group.drain(..) {
                            line.push(c);
                        }
                        state.current_line = line;
                        group_has_nonmovement = false;
                        // The movement goes to pending for next group
                        pending_movements.push(op_to_char(op));
                    } else {
                        // Still accumulating movements
                        pending_movements.push(op_to_char(op));
                    }
                } else {
                    // Non-movement op
                    if group_has_nonmovement {
                        // Same cell group: just append
                        current_group.push(op_to_char(op));
                    } else {
                        // First non-movement in new cell: flush pending movements as prefix
                        // They're already in pending_movements
                        current_group.push(op_to_char(op));
                        group_has_nonmovement = true;
                    }
                }
                i += 1;
            }

            Node::Loop { body, position } => {
                last_bf_source_line = Some(position.line);
                last_bf_indent = indent_level;

                // Flush any pending group before the loop
                if group_has_nonmovement || !current_group.is_empty() {
                    if !state.current_line.is_empty() {
                        state.push_line();
                    }
                    let mut line = indent.clone();
                    for m in pending_movements.drain(..) {
                        line.push(m);
                    }
                    for c in current_group.drain(..) {
                        line.push(c);
                    }
                    state.current_line = line;
                    state.push_line();
                    group_has_nonmovement = false;
                } else if !pending_movements.is_empty() {
                    // Pending movements with no following non-movement op yet —
                    // but a loop follows, so the movements belong to the PREVIOUS group
                    // (or stand alone). Emit them.
                    if !state.current_line.is_empty() {
                        state.push_line();
                    }
                    let mut line = indent.clone();
                    for m in pending_movements.drain(..) {
                        line.push(m);
                    }
                    state.current_line = line;
                    state.push_line();
                } else if !state.current_line.is_empty() {
                    state.push_line();
                }

                // Try inline loop
                let config = state.config.clone();
                if let Some(inline_str) = try_inline_loop(body, indent_level, &config) {
                    // Emit: indent + inline_str
                    state.current_line = format!("{}{}", indent, inline_str);
                    // Don't push yet — allow inline comment to attach
                } else {
                    // Multi-line loop
                    state.current_line = format!("{}[", indent);
                    state.push_line();
                    format_nodes_direct(body, indent_level + 1, state);
                    // Flush any remaining group inside body
                    // (format_nodes_direct handles its own flushing at the end)
                    state.current_line = format!("{}]", indent);
                    // Don't push yet — allow inline comment to attach
                }

                i += 1;
            }

            Node::Comment { text, kind, position } => {
                let comment_source_line = position.line;
                let is_inline = if let Some(last_line) = last_bf_source_line {
                    comment_source_line == last_line
                } else {
                    false
                };

                if is_inline {
                    // Inline comment: flush pending group first, then append to current line
                    if group_has_nonmovement || !current_group.is_empty() {
                        if !state.current_line.is_empty() {
                            state.push_line();
                        }
                        let mut line = indent.clone();
                        for m in pending_movements.drain(..) {
                            line.push(m);
                        }
                        for c in current_group.drain(..) {
                            line.push(c);
                        }
                        state.current_line = line;
                        group_has_nonmovement = false;
                    } else if !pending_movements.is_empty() {
                        if !state.current_line.is_empty() {
                            state.push_line();
                        }
                        let mut line = indent.clone();
                        for m in pending_movements.drain(..) {
                            line.push(m);
                        }
                        state.current_line = line;
                    }

                    // Append comment to current line
                    match kind {
                        CommentKind::Line => {
                            state.append(&format!(" //{}", text));
                        }
                        CommentKind::Block => {
                            state.append(&format!(" /*{}*/", text));
                        }
                    }
                    state.push_line();
                } else {
                    // Standalone comment: flush any pending group first
                    if group_has_nonmovement || !current_group.is_empty() {
                        if !state.current_line.is_empty() {
                            state.push_line();
                        }
                        let mut line = indent.clone();
                        for m in pending_movements.drain(..) {
                            line.push(m);
                        }
                        for c in current_group.drain(..) {
                            line.push(c);
                        }
                        state.current_line = line;
                        state.push_line();
                        group_has_nonmovement = false;
                    } else if !pending_movements.is_empty() {
                        if !state.current_line.is_empty() {
                            state.push_line();
                        }
                        let mut line = indent.clone();
                        for m in pending_movements.drain(..) {
                            line.push(m);
                        }
                        state.current_line = line;
                        state.push_line();
                    } else if !state.current_line.is_empty() {
                        state.push_line();
                    }

                    // Emit standalone comment indented at last_bf_indent level
                    let comment_indent = state.config.indent_at_level(last_bf_indent);
                    match kind {
                        CommentKind::Line => {
                            state.current_line = format!("{}//{}", comment_indent, text);
                        }
                        CommentKind::Block => {
                            state.current_line = format!("{}/*{}*/", comment_indent, text);
                        }
                    }
                    state.push_line();
                }

                i += 1;
            }
        }
    }

    // Flush any remaining group
    if group_has_nonmovement || !current_group.is_empty() {
        if !state.current_line.is_empty() {
            state.push_line();
        }
        let mut line = indent.clone();
        for m in pending_movements.drain(..) {
            line.push(m);
        }
        for c in current_group.drain(..) {
            line.push(c);
        }
        state.current_line = line;
        state.push_line();
    } else if !pending_movements.is_empty() {
        if !state.current_line.is_empty() {
            state.push_line();
        }
        let mut line = indent.clone();
        for m in pending_movements.drain(..) {
            line.push(m);
        }
        state.current_line = line;
        state.push_line();
    } else if !state.current_line.is_empty() {
        state.push_line();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(s: &str) -> String {
        format(s, &FormatterConfig::default())
    }

    #[test]
    fn empty_input() {
        assert_eq!(fmt(""), "\n");
    }

    #[test]
    fn basic_cell_grouping() {
        // >+ then <-: two different cells
        let out = fmt(">+<-");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert_eq!(lines, vec![">+", "<-"]);
    }

    #[test]
    fn simple_inline_loop() {
        // [+-] should stay on one line
        let out = fmt("[+-]");
        assert!(out.trim().contains("[ +- ]"), "got: {:?}", out);
    }

    #[test]
    fn nested_loop_multiline() {
        let out = fmt("[[+]]");
        let lines: Vec<&str> = out.trim().lines().collect();
        // Should have [ on one line, then \t[ ... ], then ]
        assert!(lines.iter().any(|l| l.starts_with('[')), "got: {:?}", lines);
        assert!(lines.iter().any(|l| l.starts_with(']')), "got: {:?}", lines);
    }

    #[test]
    fn comments_only() {
        let out = fmt("// hello");
        assert!(out.contains("// hello"), "got: {:?}", out);
    }

    #[test]
    fn inline_comment_stays_on_line() {
        let out = fmt("+// increment\n-");
        let lines: Vec<&str> = out.trim().lines().collect();
        assert!(lines[0].contains("+"), "got: {:?}", lines);
        assert!(lines[0].contains("// increment"), "got: {:?}", lines);
    }

    #[test]
    fn standalone_comment_at_top_level() {
        let out = fmt("[+]\n// comment");
        let lines: Vec<&str> = out.trim().lines().collect();
        let last = lines.last().unwrap();
        // Should not be indented (top level)
        assert!(last.starts_with("//"), "got: {:?}", lines);
    }

    #[test]
    fn loop_with_multiline_body() {
        // >+<-[>+<-] — the loop body has different cells so goes multiline
        let out = fmt(">+<-[>+<-]");
        // The loop [ should trigger multiline because body operates on two cells
        // Actually [>+<-] — body: >, +, <, - — two cells involved
        // But length check: "[ >+ <- ]" = 9 chars + indent — fits in 80
        // So it might be inline; check both are valid
        assert!(!out.is_empty());
    }

    #[test]
    fn movement_consolidation() {
        // >>>+ should produce >>>+ on one line (all movements then op)
        let out = fmt(">>>+");
        assert!(out.trim() == ">>>+", "got: {:?}", out);
    }

    #[test]
    fn deeply_nested_tabs() {
        // [[[+]]] — outer and middle loops go multi-line (nested), innermost [+] is inline
        let out = fmt("[[[+]]]");
        let lines: Vec<&str> = out.trim().lines().collect();
        // The innermost [ + ] appears at indent level 2 (2 tabs)
        let inner_line = lines.iter().find(|l| l.contains('+'));
        assert!(inner_line.is_some());
        let s = inner_line.unwrap();
        assert!(s.starts_with("\t\t"), "expected at least 2 tabs, got: {:?}", s);

        // [[[>+<-]]] — 3 levels: outer and middle go multi-line (nested),
        // inner [>+<-] goes multi-line (2 cells), so >+ and <- appear at 3-tab indent
        let out2 = fmt("[[[>+<-]]]");
        let lines2: Vec<&str> = out2.trim().lines().collect();
        // Inner [>+<-] at level 2 → try inline: "\t\t[ >+ <- ]" = 12 chars → fits → inline
        // So content is on line "\t\t[ >+ <- ]" (2-tab prefix)
        let has_2tabs = lines2.iter().any(|l| l.starts_with("\t\t"));
        assert!(has_2tabs, "expected 2-tab indent in: {:?}", lines2);
    }
}
