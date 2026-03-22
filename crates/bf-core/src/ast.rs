use crate::lexer::{CommentKind, Position};

#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    Plus,
    Minus,
    Left,
    Right,
    Input,
    Output,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Instruction {
        op: Op,
        position: Position,
    },
    Loop {
        body: Vec<Node>,
        position: Position,
    },
    Comment {
        text: String,
        kind: CommentKind,
        position: Position,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub nodes: Vec<Node>,
}
