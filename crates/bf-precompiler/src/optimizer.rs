use bf_core::ast::{Node, Op, Program};

/// Optimized IR for the precompiler.
#[derive(Debug, Clone, PartialEq)]
pub enum OptimizedOp {
    Add(u8),
    Sub(u8),
    Left(usize),
    Right(usize),
    Read,
    Write,
    Loop(Vec<OptimizedOp>),
    /// `[-]` — set current cell to zero.
    ZeroCell,
    /// `[->+<]` — add cell[ptr] to cell[ptr + offset], zero cell[ptr].
    MoveRight(usize),
    /// `[-<+>]` — add cell[ptr] to cell[ptr - offset], zero cell[ptr].
    MoveLeft(usize),
    /// `[->N+<]` — multiply cell[ptr] by N, add to cell[ptr + offset], zero cell[ptr].
    MultiplyRight(usize, u8),
    /// `[>]` — move right until a zero cell.
    FindZeroRight,
    /// `[<]` — move left until a zero cell.
    FindZeroLeft,
}

/// Optimize a `Program` into a flat vector of `OptimizedOp`.
pub fn optimize(program: &Program) -> Vec<OptimizedOp> {
    optimize_nodes(&program.nodes)
}

fn optimize_nodes(nodes: &[Node]) -> Vec<OptimizedOp> {
    let mut result: Vec<OptimizedOp> = Vec::new();
    for node in nodes {
        match node {
            Node::Comment { .. } => {}
            Node::Instruction { op, .. } => {
                let new_op = match op {
                    Op::Plus => OptimizedOp::Add(1),
                    Op::Minus => OptimizedOp::Sub(1),
                    Op::Left => OptimizedOp::Left(1),
                    Op::Right => OptimizedOp::Right(1),
                    Op::Input => OptimizedOp::Read,
                    Op::Output => OptimizedOp::Write,
                };
                // Constant folding: merge consecutive Add/Sub/Left/Right.
                if let Some(last) = result.last_mut() {
                    match (last, &new_op) {
                        (OptimizedOp::Add(n), OptimizedOp::Add(1)) => {
                            *n = n.wrapping_add(1);
                            continue;
                        }
                        (OptimizedOp::Sub(n), OptimizedOp::Sub(1)) => {
                            *n = n.wrapping_add(1);
                            continue;
                        }
                        (OptimizedOp::Left(n), OptimizedOp::Left(1)) => {
                            *n += 1;
                            continue;
                        }
                        (OptimizedOp::Right(n), OptimizedOp::Right(1)) => {
                            *n += 1;
                            continue;
                        }
                        _ => {}
                    }
                }
                result.push(new_op);
            }
            Node::Loop { body, .. } => {
                let body_ops = optimize_nodes(body);
                let optimized = match_loop_pattern(&body_ops).unwrap_or(OptimizedOp::Loop(body_ops));
                result.push(optimized);
            }
        }
    }
    result
}

/// Try to recognize a known loop pattern from §1.7.
fn match_loop_pattern(body: &[OptimizedOp]) -> Option<OptimizedOp> {
    // [-]  →  ZeroCell
    if body == [OptimizedOp::Sub(1)] {
        return Some(OptimizedOp::ZeroCell);
    }

    // [>]  →  FindZeroRight
    if body == [OptimizedOp::Right(1)] {
        return Some(OptimizedOp::FindZeroRight);
    }

    // [<]  →  FindZeroLeft
    if body == [OptimizedOp::Left(1)] {
        return Some(OptimizedOp::FindZeroLeft);
    }

    // [->+<]  →  MoveRight(1)
    // Pattern: Sub(1), Right(n), Add(m), Left(n)  where n==offset, m==factor
    if body.len() == 4 {
        if let (
            OptimizedOp::Sub(1),
            OptimizedOp::Right(offset),
            OptimizedOp::Add(factor),
            OptimizedOp::Left(back),
        ) = (&body[0], &body[1], &body[2], &body[3])
        {
            if offset == back {
                if *factor == 1 {
                    return Some(OptimizedOp::MoveRight(*offset));
                } else {
                    return Some(OptimizedOp::MultiplyRight(*offset, *factor));
                }
            }
        }

        // [-<+>]  →  MoveLeft(1)
        if let (
            OptimizedOp::Sub(1),
            OptimizedOp::Left(offset),
            OptimizedOp::Add(1),
            OptimizedOp::Right(back),
        ) = (&body[0], &body[1], &body[2], &body[3])
        {
            if offset == back {
                return Some(OptimizedOp::MoveLeft(*offset));
            }
        }
    }

    // [->>+<<]  →  MoveRight(2) (offset can be any n for Right(n)/Left(n) with factor 1)
    // Already handled above by the 4-op pattern with Right(n)/Left(n).

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use bf_core::lexer::tokenize;
    use bf_core::parser::parse;

    fn opt(src: &str) -> Vec<OptimizedOp> {
        let tokens = tokenize(src);
        let program = parse(&tokens).unwrap();
        optimize(&program)
    }

    #[test]
    fn constant_folding_add() {
        assert_eq!(opt("+++"), vec![OptimizedOp::Add(3)]);
    }

    #[test]
    fn no_cross_cell_folding() {
        // >+>+>+ should NOT fold across the Right moves
        assert_eq!(
            opt(">+>+>+"),
            vec![
                OptimizedOp::Right(1), OptimizedOp::Add(1),
                OptimizedOp::Right(1), OptimizedOp::Add(1),
                OptimizedOp::Right(1), OptimizedOp::Add(1),
            ]
        );
    }

    #[test]
    fn zero_cell_pattern() {
        assert_eq!(opt("[-]"), vec![OptimizedOp::ZeroCell]);
    }

    #[test]
    fn move_right_pattern() {
        assert_eq!(opt("[->+<]"), vec![OptimizedOp::MoveRight(1)]);
    }

    #[test]
    fn move_right_2_pattern() {
        assert_eq!(opt("[->>+<<]"), vec![OptimizedOp::MoveRight(2)]);
    }

    #[test]
    fn move_left_pattern() {
        assert_eq!(opt("[-<+>]"), vec![OptimizedOp::MoveLeft(1)]);
    }

    #[test]
    fn multiply_right_pattern() {
        assert_eq!(opt("[->++<]"), vec![OptimizedOp::MultiplyRight(1, 2)]);
    }

    #[test]
    fn find_zero_right() {
        assert_eq!(opt("[>]"), vec![OptimizedOp::FindZeroRight]);
    }

    #[test]
    fn find_zero_left() {
        assert_eq!(opt("[<]"), vec![OptimizedOp::FindZeroLeft]);
    }

    #[test]
    fn unrecognized_loop_stays_as_loop() {
        let ops = opt("[+]");
        assert!(matches!(ops[0], OptimizedOp::Loop(_)));
    }
}
