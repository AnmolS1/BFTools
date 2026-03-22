use crate::ast::{Node, Op, Program};
use crate::lexer::{Position, Token, TokenKind};

#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    UnmatchedOpen { position: Position },
    UnmatchedClose { position: Position },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::UnmatchedOpen { position } => {
                write!(f, "Unmatched '[' at {}:{}", position.line, position.column)
            }
            ParseError::UnmatchedClose { position } => {
                write!(f, "Unmatched ']' at {}:{}", position.line, position.column)
            }
        }
    }
}

pub fn parse(tokens: &[Token]) -> Result<Program, ParseError> {
    let mut idx = 0;
    let nodes = parse_nodes(tokens, &mut idx, false)?;
    Ok(Program { nodes })
}

fn parse_nodes(
    tokens: &[Token],
    idx: &mut usize,
    inside_loop: bool,
) -> Result<Vec<Node>, ParseError> {
    let mut nodes = Vec::new();

    while *idx < tokens.len() {
        let token = &tokens[*idx];
        match &token.kind {
            TokenKind::Plus => {
                nodes.push(Node::Instruction { op: Op::Plus, position: token.position.clone() });
                *idx += 1;
            }
            TokenKind::Minus => {
                nodes.push(Node::Instruction { op: Op::Minus, position: token.position.clone() });
                *idx += 1;
            }
            TokenKind::Left => {
                nodes.push(Node::Instruction { op: Op::Left, position: token.position.clone() });
                *idx += 1;
            }
            TokenKind::Right => {
                nodes.push(Node::Instruction { op: Op::Right, position: token.position.clone() });
                *idx += 1;
            }
            TokenKind::Input => {
                nodes.push(Node::Instruction { op: Op::Input, position: token.position.clone() });
                *idx += 1;
            }
            TokenKind::Output => {
                nodes.push(Node::Instruction { op: Op::Output, position: token.position.clone() });
                *idx += 1;
            }
            TokenKind::LoopStart => {
                let loop_pos = token.position.clone();
                *idx += 1;
                let body = parse_nodes(tokens, idx, true)?;
                // Expect LoopEnd
                if *idx >= tokens.len() {
                    return Err(ParseError::UnmatchedOpen { position: loop_pos });
                }
                if tokens[*idx].kind != TokenKind::LoopEnd {
                    return Err(ParseError::UnmatchedOpen { position: loop_pos });
                }
                *idx += 1;
                nodes.push(Node::Loop { body, position: loop_pos });
            }
            TokenKind::LoopEnd => {
                if inside_loop {
                    // Signal caller to stop and consume the ']'
                    return Ok(nodes);
                } else {
                    return Err(ParseError::UnmatchedClose { position: token.position.clone() });
                }
            }
            TokenKind::Comment { text, kind } => {
                nodes.push(Node::Comment {
                    text: text.clone(),
                    kind: kind.clone(),
                    position: token.position.clone(),
                });
                *idx += 1;
            }
            TokenKind::Invalid(_) => {
                // Skip invalid tokens; validator reports them
                *idx += 1;
            }
        }
    }

    Ok(nodes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;

    #[test]
    fn two_instructions() {
        let tokens = tokenize(">+");
        let prog = parse(&tokens).unwrap();
        assert_eq!(prog.nodes.len(), 2);
        assert!(matches!(prog.nodes[0], Node::Instruction { op: Op::Right, .. }));
        assert!(matches!(prog.nodes[1], Node::Instruction { op: Op::Plus, .. }));
    }

    #[test]
    fn simple_loop() {
        let tokens = tokenize("[+-]");
        let prog = parse(&tokens).unwrap();
        assert_eq!(prog.nodes.len(), 1);
        if let Node::Loop { body, .. } = &prog.nodes[0] {
            assert_eq!(body.len(), 2);
        } else {
            panic!("expected Loop");
        }
    }

    #[test]
    fn nested_loops() {
        let tokens = tokenize("[[+]]");
        let prog = parse(&tokens).unwrap();
        assert_eq!(prog.nodes.len(), 1);
        if let Node::Loop { body, .. } = &prog.nodes[0] {
            assert_eq!(body.len(), 1);
            assert!(matches!(body[0], Node::Loop { .. }));
        } else {
            panic!("expected outer Loop");
        }
    }

    #[test]
    fn comment_in_ast() {
        let tokens = tokenize("+ // hi\n-");
        let prog = parse(&tokens).unwrap();
        assert_eq!(prog.nodes.len(), 3);
        assert!(matches!(prog.nodes[1], Node::Comment { .. }));
    }

    #[test]
    fn unmatched_open() {
        let tokens = tokenize("[");
        assert!(matches!(parse(&tokens), Err(ParseError::UnmatchedOpen { .. })));
    }

    #[test]
    fn unmatched_close() {
        let tokens = tokenize("]");
        assert!(matches!(parse(&tokens), Err(ParseError::UnmatchedClose { .. })));
    }

    #[test]
    fn complex_program() {
        // >+[-<+>]
        let tokens = tokenize(">+[-<+>]");
        let prog = parse(&tokens).unwrap();
        assert_eq!(prog.nodes.len(), 3);
        if let Node::Loop { body, .. } = &prog.nodes[2] {
            assert_eq!(body.len(), 4);
        } else {
            panic!("expected loop at index 2");
        }
    }

    #[test]
    fn skip_invalid_tokens() {
        let tokens = tokenize("+@-");
        let prog = parse(&tokens).unwrap();
        // Invalid '@' is skipped
        assert_eq!(prog.nodes.len(), 2);
    }
}
