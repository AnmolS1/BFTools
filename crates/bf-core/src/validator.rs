use crate::ast::{Node, Op};
use crate::lexer::{tokenize, Position, TokenKind};
use crate::parser::parse;

#[derive(Debug, Clone, PartialEq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub rule: String,
    pub severity: Severity,
    pub position: Position,
    pub message: String,
}

pub fn validate(input: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let tokens = tokenize(input);

    // BF001: Invalid characters
    for token in &tokens {
        if let TokenKind::Invalid(ch) = token.kind {
            diagnostics.push(Diagnostic {
                rule: "BF001".to_string(),
                severity: Severity::Error,
                position: token.position.clone(),
                message: format!("Invalid character '{}'", ch),
            });
        }
    }

    // BF002 & BF003: Unmatched brackets via stack
    {
        let mut stack: Vec<Position> = Vec::new();
        for token in &tokens {
            match &token.kind {
                TokenKind::LoopStart => {
                    stack.push(token.position.clone());
                }
                TokenKind::LoopEnd => {
                    if stack.is_empty() {
                        diagnostics.push(Diagnostic {
                            rule: "BF003".to_string(),
                            severity: Severity::Error,
                            position: token.position.clone(),
                            message: "Unmatched ']'".to_string(),
                        });
                    } else {
                        stack.pop();
                    }
                }
                _ => {}
            }
        }
        for pos in stack {
            diagnostics.push(Diagnostic {
                rule: "BF002".to_string(),
                severity: Severity::Error,
                position: pos,
                message: "Unmatched '['".to_string(),
            });
        }
    }

    // BF004 & BF006: Need AST; only proceed if parse succeeds
    if let Ok(program) = parse(&tokens) {
        check_ast_rules(&program.nodes, &mut diagnostics);
    }

    // BF005: Adjacent +/- or -/+ pairs in token stream (comments filtered)
    {
        let bf_tokens: Vec<_> = tokens
            .iter()
            .filter(|t| !matches!(t.kind, TokenKind::Comment { .. } | TokenKind::Invalid(_)))
            .collect();
        for window in bf_tokens.windows(2) {
            let a = &window[0].kind;
            let b = &window[1].kind;
            if (a == &TokenKind::Plus && b == &TokenKind::Minus)
                || (a == &TokenKind::Minus && b == &TokenKind::Plus)
            {
                diagnostics.push(Diagnostic {
                    rule: "BF005".to_string(),
                    severity: Severity::Info,
                    position: window[0].position.clone(),
                    message: "Suspicious pattern: +- or -+ cancels out".to_string(),
                });
            }
        }
    }

    diagnostics
}

fn check_ast_rules(nodes: &[Node], diagnostics: &mut Vec<Diagnostic>) {
    let mut i = 0;
    while i < nodes.len() {
        match &nodes[i] {
            Node::Loop { body, position } => {
                // BF004: Empty loop (only comments in body counts as effectively empty)
                let has_bf = body.iter().any(|n| matches!(n, Node::Instruction { .. } | Node::Loop { .. }));
                if !has_bf {
                    diagnostics.push(Diagnostic {
                        rule: "BF004".to_string(),
                        severity: Severity::Warning,
                        position: position.clone(),
                        message: "Empty loop body (infinite loop if cell is non-zero)".to_string(),
                    });
                }

                // BF006: [-] followed by [...] with no intervening +, ,, or movement
                // Check if this is a [-] pattern
                if is_zero_cell_loop(body) {
                    // Look at next non-comment node
                    let next_bf = nodes[i + 1..].iter().find(|n| !matches!(n, Node::Comment { .. }));
                    if let Some(Node::Loop { .. }) = next_bf {
                        // Check no intervening +, ,, or movement between this loop and the next
                        let mut j = i + 1;
                        let mut only_comments = true;
                        while j < nodes.len() {
                            match &nodes[j] {
                                Node::Comment { .. } => {}
                                Node::Loop { .. } => break,
                                Node::Instruction { op, .. } => {
                                    match op {
                                        Op::Plus | Op::Minus | Op::Input | Op::Left | Op::Right => {
                                            only_comments = false;
                                            break;
                                        }
                                        Op::Output => {}
                                    }
                                }
                            }
                            j += 1;
                        }
                        if only_comments {
                            diagnostics.push(Diagnostic {
                                rule: "BF006".to_string(),
                                severity: Severity::Info,
                                position: position.clone(),
                                message: "[-] followed by loop with no intervening modification (dead code pattern)".to_string(),
                            });
                        }
                    }
                }

                // Recurse into loop body
                check_ast_rules(body, diagnostics);
            }
            _ => {}
        }
        i += 1;
    }
}

fn is_zero_cell_loop(body: &[Node]) -> bool {
    // [-] pattern: exactly one Minus instruction (ignoring comments)
    let bf_nodes: Vec<_> = body.iter().filter(|n| !matches!(n, Node::Comment { .. })).collect();
    bf_nodes.len() == 1 && matches!(bf_nodes[0], Node::Instruction { op: Op::Minus, .. })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bf001_invalid_char() {
        let diags = validate("+@-");
        assert!(diags.iter().any(|d| d.rule == "BF001"));
    }

    #[test]
    fn bf002_unmatched_open() {
        let diags = validate("[+");
        assert!(diags.iter().any(|d| d.rule == "BF002"));
    }

    #[test]
    fn bf003_unmatched_close() {
        let diags = validate("+]");
        assert!(diags.iter().any(|d| d.rule == "BF003"));
    }

    #[test]
    fn bf004_empty_loop() {
        let diags = validate("[]");
        assert!(diags.iter().any(|d| d.rule == "BF004"));
    }

    #[test]
    fn bf005_plus_minus() {
        let diags = validate("+-");
        assert!(diags.iter().any(|d| d.rule == "BF005"));
    }

    #[test]
    fn bf005_minus_plus() {
        let diags = validate("-+");
        assert!(diags.iter().any(|d| d.rule == "BF005"));
    }

    #[test]
    fn bf006_dead_code_after_zero() {
        let diags = validate("[-][+]");
        assert!(diags.iter().any(|d| d.rule == "BF006"));
    }

    #[test]
    fn clean_program_no_diagnostics() {
        let diags = validate(">+[-<+>]");
        assert!(diags.is_empty(), "Expected no diagnostics, got: {:?}", diags);
    }
}
