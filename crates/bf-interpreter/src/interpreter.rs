use std::io::{Read, Write};

use bf_core::ast::{Node, Op, Program};
use bf_core::lexer::Position;

use crate::memory::{Memory, TAPE_SIZE};

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeError {
    PointerUnderflow,
    PointerOverflow,
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::PointerUnderflow => {
                write!(f, "pointer underflow: moved left past cell 0")
            }
            RuntimeError::PointerOverflow => {
                write!(f, "pointer overflow: moved right past cell {}", TAPE_SIZE - 1)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Opcode {
    Add,
    Sub,
    Left,
    Right,
    Read,
    Write,
    JumpIfZero(usize),     // [  → if cell == 0 jump to target
    JumpIfNotZero(usize),  // ]  → if cell != 0 jump to target (the JumpIfZero)
}

#[derive(Debug, Clone)]
pub struct Instruction {
    pub opcode: Opcode,
    pub source_pos: Position,
}

#[derive(Debug)]
pub enum StepResult {
    Continue,
    Output(u8),
    NeedInput, // reserved for interactive DAP use
    Finished,
    RuntimeError(RuntimeError),
}

fn compile_nodes(nodes: &[Node], instructions: &mut Vec<Instruction>) {
    for node in nodes {
        match node {
            Node::Instruction { op, position } => {
                let opcode = match op {
                    Op::Plus => Opcode::Add,
                    Op::Minus => Opcode::Sub,
                    Op::Left => Opcode::Left,
                    Op::Right => Opcode::Right,
                    Op::Input => Opcode::Read,
                    Op::Output => Opcode::Write,
                };
                instructions.push(Instruction { opcode, source_pos: position.clone() });
            }
            Node::Loop { body, position } => {
                let loop_start = instructions.len();
                // Placeholder; backpatched after body is compiled
                instructions.push(Instruction {
                    opcode: Opcode::JumpIfZero(0),
                    source_pos: position.clone(),
                });
                compile_nodes(body, instructions);
                let loop_end = instructions.len();
                // ] jumps back to the [ to re-check the condition
                instructions.push(Instruction {
                    opcode: Opcode::JumpIfNotZero(loop_start),
                    source_pos: position.clone(),
                });
                // Backpatch: [ jumps to instruction after ]
                instructions[loop_start].opcode = Opcode::JumpIfZero(loop_end + 1);
            }
            Node::Comment { .. } => {}
        }
    }
}

pub fn compile(program: &Program) -> Vec<Instruction> {
    let mut instructions = Vec::new();
    compile_nodes(&program.nodes, &mut instructions);
    instructions
}

pub struct Interpreter {
    instructions: Vec<Instruction>,
    pc: usize,
    memory: Memory,
}

impl Interpreter {
    pub fn new(program: &Program) -> Self {
        Interpreter {
            instructions: compile(program),
            pc: 0,
            memory: Memory::new(),
        }
    }

    pub fn step(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> StepResult {
        if self.pc >= self.instructions.len() {
            return StepResult::Finished;
        }

        // Copy opcode to release the borrow on self.instructions before mutating self
        let opcode = self.instructions[self.pc].opcode;

        match opcode {
            Opcode::Add => {
                self.memory.inc();
                self.pc += 1;
                StepResult::Continue
            }
            Opcode::Sub => {
                self.memory.dec();
                self.pc += 1;
                StepResult::Continue
            }
            Opcode::Left => {
                if self.memory.pointer() == 0 {
                    return StepResult::RuntimeError(RuntimeError::PointerUnderflow);
                }
                self.memory.left();
                self.pc += 1;
                StepResult::Continue
            }
            Opcode::Right => {
                if self.memory.pointer() + 1 >= TAPE_SIZE {
                    return StepResult::RuntimeError(RuntimeError::PointerOverflow);
                }
                self.memory.right();
                self.pc += 1;
                StepResult::Continue
            }
            Opcode::Read => {
                let mut buf = [0u8; 1];
                let val = match input.read(&mut buf) {
                    Ok(1) => buf[0],
                    _ => 0, // EOF or error → 0
                };
                self.memory.set(val);
                self.pc += 1;
                StepResult::Continue
            }
            Opcode::Write => {
                let b = self.memory.get();
                let _ = output.write_all(&[b]);
                self.pc += 1;
                StepResult::Output(b)
            }
            Opcode::JumpIfZero(target) => {
                if self.memory.get() == 0 {
                    self.pc = target;
                } else {
                    self.pc += 1;
                }
                StepResult::Continue
            }
            Opcode::JumpIfNotZero(target) => {
                if self.memory.get() != 0 {
                    self.pc = target;
                } else {
                    self.pc += 1;
                }
                StepResult::Continue
            }
        }
    }

    pub fn run(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<(), RuntimeError> {
        loop {
            match self.step(input, output) {
                StepResult::Continue | StepResult::Output(_) | StepResult::NeedInput => {}
                StepResult::Finished => return Ok(()),
                StepResult::RuntimeError(e) => return Err(e),
            }
        }
    }

    pub fn get_pointer(&self) -> usize {
        self.memory.pointer()
    }

    pub fn get_memory(&self) -> &[u8] {
        self.memory.cells()
    }

    pub fn is_finished(&self) -> bool {
        self.pc >= self.instructions.len()
    }

    pub fn get_pc(&self) -> usize {
        self.pc
    }

    pub fn get_instructions(&self) -> &[Instruction] {
        &self.instructions
    }

    pub fn current_source_pos(&self) -> Option<&Position> {
        self.instructions.get(self.pc).map(|i| &i.source_pos)
    }
}

#[cfg(feature = "jit")]
pub mod auto_jit {
    use super::*;
    use crate::jit::{JitCompiler, JitFn, JitIoContext};
    use std::collections::HashMap;

    const JIT_THRESHOLD: usize = 3;

    // Bridges Rust Read/Write to JIT C callbacks.
    // Fat pointers (*mut dyn Trait) are decomposed into (data, vtable) pairs to
    // avoid lifetime parameters on the struct (which would prevent use in C trampolines).
    // SAFETY: caller must ensure the trait objects outlive this context.
    #[repr(C)]
    struct TraitIoCtx {
        base: JitIoContext,
        input_data: *mut (),
        input_vtable: *const (),
        output_data: *mut (),
        output_vtable: *const (),
    }

    impl TraitIoCtx {
        fn new(input: &mut dyn Read, output: &mut dyn Write) -> Self {
            // Decompose fat pointers into (data, vtable) pairs.
            // *mut dyn Trait and (*mut (), *const ()) are both 2×usize — transmute is sound.
            let (input_data, input_vtable): (*mut (), *const ()) =
                unsafe { std::mem::transmute(input as *mut dyn Read) };
            let (output_data, output_vtable): (*mut (), *const ()) =
                unsafe { std::mem::transmute(output as *mut dyn Write) };
            TraitIoCtx {
                base: JitIoContext { write_byte: cb_write, read_byte: cb_read },
                input_data,
                input_vtable,
                output_data,
                output_vtable,
            }
        }
    }

    extern "C" fn cb_write(ctx: *mut JitIoContext, byte: u8) {
        let ctx = unsafe { &*(ctx as *const TraitIoCtx) };
        let output: *mut dyn Write =
            unsafe { std::mem::transmute((ctx.output_data, ctx.output_vtable)) };
        let _ = unsafe { (*output).write_all(&[byte]) };
    }

    extern "C" fn cb_read(ctx: *mut JitIoContext) -> u8 {
        let ctx = unsafe { &*(ctx as *const TraitIoCtx) };
        let input: *mut dyn Read =
            unsafe { std::mem::transmute((ctx.input_data, ctx.input_vtable)) };
        let mut buf = [0u8; 1];
        match unsafe { (*input).read(&mut buf) } {
            Ok(1) => buf[0],
            _ => 0,
        }
    }

    pub struct AutoJitInterpreter {
        instructions: Vec<Instruction>,
        pc: usize,
        memory: Memory,
        loop_counts: HashMap<usize, usize>,
        jit_cache: HashMap<usize, JitFn>,
        compiler: JitCompiler,
    }

    impl AutoJitInterpreter {
        pub fn new(program: &bf_core::ast::Program) -> Result<Self, String> {
            Ok(AutoJitInterpreter {
                instructions: compile(program),
                pc: 0,
                memory: Memory::new(),
                loop_counts: HashMap::new(),
                jit_cache: HashMap::new(),
                compiler: JitCompiler::new()?,
            })
        }

        pub fn run(&mut self, input: &mut dyn Read, output: &mut dyn Write) -> Result<(), RuntimeError> {
            let mut ctx = TraitIoCtx::new(input, output);

            loop {
                if self.pc >= self.instructions.len() {
                    return Ok(());
                }

                let opcode = self.instructions[self.pc].opcode;

                if let Opcode::JumpIfZero(target) = opcode {
                    if self.memory.get() != 0 {
                        let loop_pc = self.pc;
                        let count = self.loop_counts.entry(loop_pc).or_insert(0);
                        *count += 1;
                        let cur_count = *count;

                        if let Some(&jit_fn) = self.jit_cache.get(&loop_pc) {
                            let mem_ptr = self.memory.cells_as_mut_ptr();
                            let init_ptr = self.memory.pointer();
                            let final_ptr = unsafe { jit_fn(mem_ptr, init_ptr, &mut ctx.base) };
                            self.memory.set_pointer(final_ptr);
                            self.pc = target;
                            continue;
                        }

                        if cur_count >= JIT_THRESHOLD {
                            let loop_instrs = self.instructions[loop_pc..target].to_vec();
                            match self.compiler.compile(&loop_instrs) {
                                Ok(jit_fn) => {
                                    eprintln!("JIT compiled loop at offset {loop_pc}");
                                    self.jit_cache.insert(loop_pc, jit_fn);
                                    let mem_ptr = self.memory.cells_as_mut_ptr();
                                    let init_ptr = self.memory.pointer();
                                    let final_ptr = unsafe { jit_fn(mem_ptr, init_ptr, &mut ctx.base) };
                                    self.memory.set_pointer(final_ptr);
                                    self.pc = target;
                                    continue;
                                }
                                Err(e) => {
                                    eprintln!("JIT compile failed for loop at {loop_pc}: {e}");
                                    self.pc += 1; // fall through to interpreter
                                }
                            }
                        } else {
                            self.pc += 1; // enter loop body normally
                        }
                    } else {
                        self.pc = target; // cell == 0, skip loop
                    }
                    continue;
                }

                match opcode {
                    Opcode::Add => { self.memory.inc(); self.pc += 1; }
                    Opcode::Sub => { self.memory.dec(); self.pc += 1; }
                    Opcode::Left => {
                        if self.memory.pointer() == 0 {
                            return Err(RuntimeError::PointerUnderflow);
                        }
                        self.memory.left();
                        self.pc += 1;
                    }
                    Opcode::Right => {
                        if self.memory.pointer() + 1 >= TAPE_SIZE {
                            return Err(RuntimeError::PointerOverflow);
                        }
                        self.memory.right();
                        self.pc += 1;
                    }
                    Opcode::Read => {
                        let val = cb_read(&mut ctx.base);
                        self.memory.set(val);
                        self.pc += 1;
                    }
                    Opcode::Write => {
                        let b = self.memory.get();
                        cb_write(&mut ctx.base, b);
                        self.pc += 1;
                    }
                    Opcode::JumpIfZero(_) => unreachable!("handled above"),
                    Opcode::JumpIfNotZero(target) => {
                        if self.memory.get() != 0 {
                            self.pc = target;
                        } else {
                            self.pc += 1;
                        }
                    }
                }
            }
        }

        pub fn get_pointer(&self) -> usize { self.memory.pointer() }
        pub fn get_memory(&self) -> &[u8] { self.memory.cells() }
        pub fn is_loop_jit_cached(&self, pc: usize) -> bool { self.jit_cache.contains_key(&pc) }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use bf_core::lexer::tokenize;
        use bf_core::parser::parse;
        use std::io::Cursor;

        fn run_auto_jit(src: &str, input: &[u8]) -> (Vec<u8>, usize, Vec<u8>) {
            let tokens = tokenize(src);
            let program = parse(&tokens).expect("parse failed");
            let mut interp = AutoJitInterpreter::new(&program).expect("init failed");
            let mut input_cur = Cursor::new(input.to_vec());
            let mut output = Vec::new();
            interp.run(&mut input_cur, &mut output).expect("run failed");
            let ptr = interp.get_pointer();
            let mem = interp.get_memory().to_vec();
            (output, ptr, mem)
        }

        #[test]
        fn auto_jit_simple_loop_correct() {
            // +++++[>+<-] runs 5 iterations; JIT triggers at iteration 3
            let (_, ptr, mem) = run_auto_jit("+++++[>+<-]", &[]);
            assert_eq!(mem[0], 0);
            assert_eq!(mem[1], 5);
            assert_eq!(ptr, 0);
        }

        #[test]
        fn auto_jit_triggers_and_caches() {
            // "+++++[>+<-]": 5 Add opcodes then JumpIfZero at pc=5
            // Loop runs 5 times; JIT triggers at iteration 3
            let tokens = tokenize("+++++[>+<-]");
            let program = parse(&tokens).expect("parse failed");
            let mut interp = AutoJitInterpreter::new(&program).expect("init failed");
            let mut input_cur = Cursor::new(vec![]);
            let mut output = Vec::new();
            interp.run(&mut input_cur, &mut output).expect("run failed");
            // JumpIfZero is at pc=5 (after 5 Add instructions)
            assert!(interp.is_loop_jit_cached(5));
        }

        #[test]
        fn auto_jit_nested_loops() {
            // ++[>++[>++<-]<-] → cell[2]=8 (2 × 2 × 2)
            let (_, _, mem) = run_auto_jit("++[>++[>++<-]<-]", &[]);
            assert_eq!(mem[2], 8);
        }

        #[test]
        fn auto_jit_semantic_equivalence() {
            // Compare auto-jit output with pure interpreter output for hello world
            let src = "++++++++[>++++[>++>+++>+++>+<<<<-]>+>+>->>+[<]<-]>>.>---.+++++++..+++.>>.<-.<.+++.------.--------.>>+.>++.";

            let tokens = tokenize(src);
            let program = parse(&tokens).expect("parse failed");

            // Pure interpreter
            let mut interp = Interpreter::new(&program);
            let mut input_cur = Cursor::new(vec![]);
            let mut interp_out = Vec::new();
            interp.run(&mut input_cur, &mut interp_out).expect("interp run failed");

            // Auto-jit
            let mut auto = AutoJitInterpreter::new(&program).expect("auto-jit init failed");
            let mut input_cur2 = Cursor::new(vec![]);
            let mut auto_out = Vec::new();
            auto.run(&mut input_cur2, &mut auto_out).expect("auto-jit run failed");

            assert_eq!(interp_out, auto_out);
        }

        #[test]
        fn auto_jit_io_works() {
            let (output, _, _) = run_auto_jit(",[.,]", b"AB");
            assert_eq!(output, b"AB");
        }
    }
}

#[cfg(feature = "jit")]
pub use auto_jit::AutoJitInterpreter;

#[cfg(test)]
mod tests {
    use super::*;
    use bf_core::lexer::tokenize;
    use bf_core::parser::parse;
    use std::io::Cursor;

    fn run_bf(src: &str, input: &[u8]) -> (Vec<u8>, Result<(), RuntimeError>) {
        let tokens = tokenize(src);
        let program = parse(&tokens).expect("parse failed");
        let mut interp = Interpreter::new(&program);
        let mut input_cursor = Cursor::new(input.to_vec());
        let mut output = Vec::new();
        let result = interp.run(&mut input_cursor, &mut output);
        (output, result)
    }

    fn get_state(src: &str, input: &[u8]) -> Interpreter {
        let tokens = tokenize(src);
        let program = parse(&tokens).expect("parse failed");
        let mut interp = Interpreter::new(&program);
        let mut input_cursor = Cursor::new(input.to_vec());
        let _ = interp.run(&mut input_cursor, &mut std::io::sink());
        interp
    }

    #[test]
    fn hello_world() {
        // Wikipedia BF hello world — known to produce "Hello World!\n"
        let src = "++++++++[>++++[>++>+++>+++>+<<<<-]>+>+>->>+[<]<-]>>.>---.+++++++..+++.>>.<-.<.+++.------.--------.>>+.>++.";
        let (output, result) = run_bf(src, &[]);
        assert!(result.is_ok());
        assert_eq!(output, b"Hello World!\n");
    }

    #[test]
    fn echo_cat() {
        let (output, result) = run_bf(",[.,]", b"hello");
        assert!(result.is_ok());
        assert_eq!(output, b"hello");
    }

    #[test]
    fn wrapping_decrement() {
        // Decrement from 0 wraps to 255
        let interp = get_state("-", &[]);
        assert_eq!(interp.get_memory()[0], 255);
    }

    #[test]
    fn wrapping_increment() {
        // 256 increments from 0 wraps back to 0
        let src = "+".repeat(256);
        let interp = get_state(&src, &[]);
        assert_eq!(interp.get_memory()[0], 0);
    }

    #[test]
    fn pointer_underflow() {
        let (_, result) = run_bf("<", &[]);
        assert!(matches!(result, Err(RuntimeError::PointerUnderflow)));
    }

    #[test]
    fn pointer_overflow() {
        let src = ">".repeat(30000);
        let (_, result) = run_bf(&src, &[]);
        assert!(matches!(result, Err(RuntimeError::PointerOverflow)));
    }

    #[test]
    fn eof_sets_zero() {
        // , with empty input → cell stays 0
        let interp = get_state(",", &[]);
        assert_eq!(interp.get_memory()[0], 0);
    }

    #[test]
    fn compile_ir_correctness() {
        let tokens = tokenize("[+-]");
        let program = parse(&tokens).unwrap();
        let instructions = compile(&program);

        // [  → JumpIfZero(4)  (jump past ] to index 4)
        // +  → Add
        // -  → Sub
        // ]  → JumpIfNotZero(0)  (jump back to [)
        assert_eq!(instructions.len(), 4);
        assert!(matches!(instructions[0].opcode, Opcode::JumpIfZero(4)));
        assert!(matches!(instructions[1].opcode, Opcode::Add));
        assert!(matches!(instructions[2].opcode, Opcode::Sub));
        assert!(matches!(instructions[3].opcode, Opcode::JumpIfNotZero(0)));
    }
}
