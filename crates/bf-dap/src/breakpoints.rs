use std::collections::{HashMap, HashSet};

use bf_interpreter::Instruction;

/// Maps 1-based source line numbers to instruction offsets (PCs) on that line.
pub fn build_source_map(instructions: &[Instruction]) -> HashMap<usize, Vec<usize>> {
    let mut map: HashMap<usize, Vec<usize>> = HashMap::new();
    for (offset, instr) in instructions.iter().enumerate() {
        map.entry(instr.source_pos.line).or_default().push(offset);
    }
    map
}

pub struct BreakpointResult {
    pub verified: bool,
    pub line: usize,
    pub message: Option<String>,
}

pub struct BreakpointManager {
    pcs: HashSet<usize>,
}

impl BreakpointManager {
    pub fn new() -> Self {
        BreakpointManager { pcs: HashSet::new() }
    }

    pub fn set_breakpoints(
        &mut self,
        source_map: &HashMap<usize, Vec<usize>>,
        lines: &[usize],
    ) -> Vec<BreakpointResult> {
        self.pcs.clear();
        lines
            .iter()
            .map(|&line| {
                if let Some(offsets) = source_map.get(&line) {
                    self.pcs.insert(offsets[0]);
                    BreakpointResult { verified: true, line, message: None }
                } else {
                    BreakpointResult {
                        verified: false,
                        line,
                        message: Some("No instruction on this line".to_string()),
                    }
                }
            })
            .collect()
    }

    pub fn is_at_breakpoint(&self, pc: usize) -> bool {
        self.pcs.contains(&pc)
    }

    pub fn clear(&mut self) {
        self.pcs.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bf_core::lexer::tokenize;
    use bf_core::parser::parse;
    use bf_interpreter::compile;

    #[test]
    fn breakpoint_on_instruction_line() {
        let tokens = tokenize("+++");
        let program = parse(&tokens).unwrap();
        let instructions = compile(&program);
        let src_map = build_source_map(&instructions);

        let mut mgr = BreakpointManager::new();
        let results = mgr.set_breakpoints(&src_map, &[1]);

        assert!(results[0].verified);
        assert!(mgr.is_at_breakpoint(0)); // first Add is at pc=0
    }

    #[test]
    fn breakpoint_on_comment_line_unverified() {
        // Line 1 has only a comment (no BF instructions); line 2 has "+"
        let tokens = tokenize("// comment\n+");
        let program = parse(&tokens).unwrap();
        let instructions = compile(&program);
        let src_map = build_source_map(&instructions);

        let mut mgr = BreakpointManager::new();
        let results = mgr.set_breakpoints(&src_map, &[1]);

        assert!(!results[0].verified);
    }

    #[test]
    fn clear_breakpoints_allows_run_to_completion() {
        let tokens = tokenize("+++");
        let program = parse(&tokens).unwrap();
        let instructions = compile(&program);
        let src_map = build_source_map(&instructions);

        let mut mgr = BreakpointManager::new();
        mgr.set_breakpoints(&src_map, &[1]);
        assert!(mgr.is_at_breakpoint(0));
        mgr.clear();
        assert!(!mgr.is_at_breakpoint(0));
    }
}
