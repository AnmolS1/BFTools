use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::types::I8;
use cranelift_codegen::ir::{
    AbiParam, Function, InstBuilder, MemFlags, Signature, UserFuncName,
};
use cranelift_codegen::isa::CallConv;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_codegen::Context;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};

use crate::interpreter::{Instruction, Opcode};

/// JIT-compiled BF function signature:
/// `fn(memory: *mut u8, initial_ptr: usize, ctx: *mut JitIoContext) -> usize`
/// Returns final pointer value.
pub type JitFn = unsafe extern "C" fn(*mut u8, usize, *mut JitIoContext) -> usize;

/// I/O context passed to JIT callbacks. The function pointers are embedded
/// so JIT code can call them via a single context pointer.
#[repr(C)]
pub struct JitIoContext {
    pub write_byte: extern "C" fn(*mut JitIoContext, u8),
    pub read_byte: extern "C" fn(*mut JitIoContext) -> u8,
    // Concrete I/O sources follow — use VecIoContext for owned I/O
}

pub extern "C" fn noop_write(_ctx: *mut JitIoContext, _b: u8) {}
pub extern "C" fn noop_read(_ctx: *mut JitIoContext) -> u8 {
    0
}

/// A concrete I/O context backed by Vec<u8> for tests.
#[repr(C)]
pub struct VecIoContext {
    base: JitIoContext,
    output: Vec<u8>,
    input: std::io::Cursor<Vec<u8>>,
}

impl VecIoContext {
    pub fn new(input: &[u8]) -> Box<Self> {
        Box::new(VecIoContext {
            base: JitIoContext {
                write_byte: vec_write,
                read_byte: vec_read,
            },
            output: Vec::new(),
            input: std::io::Cursor::new(input.to_vec()),
        })
    }

    pub fn output(&self) -> &[u8] {
        &self.output
    }
}

extern "C" fn vec_write(ctx: *mut JitIoContext, byte: u8) {
    let ctx = unsafe { &mut *(ctx as *mut VecIoContext) };
    ctx.output.push(byte);
}

extern "C" fn vec_read(ctx: *mut JitIoContext) -> u8 {
    use std::io::Read;
    let ctx = unsafe { &mut *(ctx as *mut VecIoContext) };
    let mut buf = [0u8; 1];
    match ctx.input.read(&mut buf) {
        Ok(1) => buf[0],
        _ => 0,
    }
}

/// Trampoline for write: reads fn ptr from ctx[0] and calls it.
extern "C" fn jit_write_trampoline(ctx: *mut JitIoContext, byte: u8) {
    let write_fn = unsafe { (*ctx).write_byte };
    write_fn(ctx, byte);
}

/// Trampoline for read: reads fn ptr from ctx[0] and calls it.
extern "C" fn jit_read_trampoline(ctx: *mut JitIoContext) -> u8 {
    let read_fn = unsafe { (*ctx).read_byte };
    read_fn(ctx)
}

pub struct JitCompiler {
    module: JITModule,
    counter: u32,
}

impl JitCompiler {
    pub fn new() -> Result<Self, String> {
        let mut flag_builder = settings::builder();
        flag_builder.set("use_colocated_libcalls", "false").unwrap();
        flag_builder.set("is_pic", "false").unwrap();
        let mut jit_builder = JITBuilder::new(cranelift_module::default_libcall_names())
            .map_err(|e| format!("JITBuilder: {e}"))?;
        // Register I/O trampolines so JIT code can call them
        jit_builder.symbol("bf_write", jit_write_trampoline as *const u8);
        jit_builder.symbol("bf_read", jit_read_trampoline as *const u8);

        Ok(JitCompiler {
            module: JITModule::new(jit_builder),
            counter: 0,
        })
    }

    /// Compile the given instructions to a native function starting at `start_pc`.
    pub fn compile(&mut self, instructions: &[Instruction]) -> Result<JitFn, String> {
        let ptr_type = self.module.target_config().pointer_type();

        // External function signatures
        let write_sig = {
            let mut sig = Signature::new(CallConv::SystemV);
            sig.params.push(AbiParam::new(ptr_type)); // ctx *
            sig.params.push(AbiParam::new(I8)); // byte
            sig
        };
        let read_sig = {
            let mut sig = Signature::new(CallConv::SystemV);
            sig.params.push(AbiParam::new(ptr_type)); // ctx *
            sig.returns.push(AbiParam::new(I8));
            sig
        };

        let write_id = self
            .module
            .declare_function("bf_write", Linkage::Import, &write_sig)
            .map_err(|e| format!("declare bf_write: {e}"))?;
        let read_id = self
            .module
            .declare_function("bf_read", Linkage::Import, &read_sig)
            .map_err(|e| format!("declare bf_read: {e}"))?;

        // Main function: (memory: ptr, initial_ptr: ptr, ctx: ptr) -> ptr
        let mut main_sig = Signature::new(CallConv::SystemV);
        main_sig.params.push(AbiParam::new(ptr_type)); // memory base
        main_sig.params.push(AbiParam::new(ptr_type)); // initial pointer
        main_sig.params.push(AbiParam::new(ptr_type)); // ctx
        main_sig.returns.push(AbiParam::new(ptr_type)); // final pointer

        let fn_name = format!("bf_jit_fn_{}", self.counter);
        self.counter += 1;
        let main_id = self
            .module
            .declare_function(&fn_name, Linkage::Local, &main_sig)
            .map_err(|e| format!("declare main: {e}"))?;

        let mut func = Function::with_name_signature(
            UserFuncName::user(0, main_id.as_u32()),
            main_sig,
        );

        // Declare external functions within the compiled function
        let write_ref = self.module.declare_func_in_func(write_id, &mut func);
        let read_ref = self.module.declare_func_in_func(read_id, &mut func);

        // Build the function body
        let mut builder_ctx = FunctionBuilderContext::new();
        {
            let mut builder = FunctionBuilder::new(&mut func, &mut builder_ctx);

            let v_mem = Variable::from_u32(0);
            let v_ptr = Variable::from_u32(1);
            let v_ctx = Variable::from_u32(2);

            builder.declare_var(v_mem, ptr_type);
            builder.declare_var(v_ptr, ptr_type);
            builder.declare_var(v_ctx, ptr_type);

            let entry_block = builder.create_block();
            builder.append_block_params_for_function_params(entry_block);
            builder.switch_to_block(entry_block);
            builder.seal_block(entry_block); // no predecessors

            let mem_arg = builder.block_params(entry_block)[0];
            let ptr_arg = builder.block_params(entry_block)[1];
            let ctx_arg = builder.block_params(entry_block)[2];

            builder.def_var(v_mem, mem_arg);
            builder.def_var(v_ptr, ptr_arg);
            builder.def_var(v_ctx, ctx_arg);

            emit_instructions(
                instructions,
                &mut builder,
                ptr_type,
                v_mem,
                v_ptr,
                v_ctx,
                write_ref,
                read_ref,
            )?;

            let final_ptr = builder.use_var(v_ptr);
            builder.ins().return_(&[final_ptr]);

            builder.seal_all_blocks();
            builder.finalize();
        }

        let mut ctx = Context::for_function(func);
        self.module
            .define_function(main_id, &mut ctx)
            .map_err(|e| format!("define function: {e}"))?;

        self.module
            .finalize_definitions()
            .map_err(|e| format!("finalize: {e}"))?;

        let ptr = self.module.get_finalized_function(main_id);
        Ok(unsafe { std::mem::transmute(ptr) })
    }
}

fn emit_instructions(
    instructions: &[Instruction],
    builder: &mut FunctionBuilder,
    ptr_type: cranelift_codegen::ir::Type,
    v_mem: Variable,
    v_ptr: Variable,
    v_ctx: Variable,
    write_ref: cranelift_codegen::ir::FuncRef,
    read_ref: cranelift_codegen::ir::FuncRef,
) -> Result<(), String> {
    // Stack of (cond_block, exit_block) for nested loops
    let mut loop_stack: Vec<(cranelift_codegen::ir::Block, cranelift_codegen::ir::Block)> =
        Vec::new();

    for instr in instructions {
        match instr.opcode {
            Opcode::Add => {
                let base = builder.use_var(v_mem);
                let p = builder.use_var(v_ptr);
                let addr = builder.ins().iadd(base, p);
                let val = builder.ins().load(I8, MemFlags::trusted(), addr, 0);
                let one = builder.ins().iconst(I8, 1);
                let new_val = builder.ins().iadd(val, one);
                builder.ins().store(MemFlags::trusted(), new_val, addr, 0);
            }
            Opcode::Sub => {
                let base = builder.use_var(v_mem);
                let p = builder.use_var(v_ptr);
                let addr = builder.ins().iadd(base, p);
                let val = builder.ins().load(I8, MemFlags::trusted(), addr, 0);
                let one = builder.ins().iconst(I8, 1);
                let new_val = builder.ins().isub(val, one);
                builder.ins().store(MemFlags::trusted(), new_val, addr, 0);
            }
            Opcode::Right => {
                let p = builder.use_var(v_ptr);
                let one = builder.ins().iconst(ptr_type, 1);
                let new_p = builder.ins().iadd(p, one);
                builder.def_var(v_ptr, new_p);
            }
            Opcode::Left => {
                let p = builder.use_var(v_ptr);
                let one = builder.ins().iconst(ptr_type, 1);
                let new_p = builder.ins().isub(p, one);
                builder.def_var(v_ptr, new_p);
            }
            Opcode::Write => {
                let base = builder.use_var(v_mem);
                let p = builder.use_var(v_ptr);
                let addr = builder.ins().iadd(base, p);
                let val = builder.ins().load(I8, MemFlags::trusted(), addr, 0);
                let ctx = builder.use_var(v_ctx);
                builder.ins().call(write_ref, &[ctx, val]);
            }
            Opcode::Read => {
                let ctx = builder.use_var(v_ctx);
                let call = builder.ins().call(read_ref, &[ctx]);
                let val = builder.inst_results(call)[0];
                let base = builder.use_var(v_mem);
                let p = builder.use_var(v_ptr);
                let addr = builder.ins().iadd(base, p);
                builder.ins().store(MemFlags::trusted(), val, addr, 0);
            }
            Opcode::JumpIfZero(_exit_pc) => {
                let cond_block = builder.create_block();
                let body_block = builder.create_block();
                let exit_block = builder.create_block();

                // Fall through to the condition check block
                builder.ins().jump(cond_block, &[]);

                // Condition block: check cell[ptr] != 0
                builder.switch_to_block(cond_block);
                // Do NOT seal cond_block yet — it gets a back edge from JumpIfNotZero

                let base = builder.use_var(v_mem);
                let p = builder.use_var(v_ptr);
                let addr = builder.ins().iadd(base, p);
                let val = builder.ins().load(I8, MemFlags::trusted(), addr, 0);
                let zero = builder.ins().iconst(I8, 0);
                let cond = builder.ins().icmp(IntCC::NotEqual, val, zero);
                builder.ins().brif(cond, body_block, &[], exit_block, &[]);

                // Seal body_block (only predecessor: cond_block via brif)
                builder.seal_block(body_block);
                // Seal exit_block (only predecessor: cond_block via brif)
                builder.seal_block(exit_block);

                builder.switch_to_block(body_block);

                loop_stack.push((cond_block, exit_block));
            }
            Opcode::JumpIfNotZero(_loop_start) => {
                let (cond_block, exit_block) = loop_stack
                    .pop()
                    .ok_or("JumpIfNotZero without matching JumpIfZero")?;

                // Back edge to condition block
                builder.ins().jump(cond_block, &[]);
                // Now cond_block has all predecessors; seal it
                builder.seal_block(cond_block);

                builder.switch_to_block(exit_block);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bf_core::lexer::tokenize;
    use bf_core::parser::parse;
    use crate::interpreter::compile;
    use crate::memory::TAPE_SIZE;

    fn run_jit(src: &str, input: &[u8]) -> (Vec<u8>, usize, Vec<u8>) {
        let tokens = tokenize(src);
        let program = parse(&tokens).expect("parse failed");
        let instructions = compile(&program);

        let mut memory = vec![0u8; TAPE_SIZE];
        let mut io_ctx = VecIoContext::new(input);
        let ctx_ptr = &mut *io_ctx as *mut VecIoContext as *mut JitIoContext;

        let mut compiler = JitCompiler::new().expect("JIT compiler init failed");
        let func = compiler.compile(&instructions).expect("JIT compile failed");

        let final_ptr = unsafe { func(memory.as_mut_ptr(), 0, ctx_ptr) };

        let output = io_ctx.output().to_vec();
        (output, final_ptr, memory)
    }

    #[test]
    fn jit_move_and_increment() {
        let (_, ptr, mem) = run_jit(">+++", &[]);
        assert_eq!(ptr, 1);
        assert_eq!(mem[1], 3);
    }

    #[test]
    fn jit_write_output() {
        let src = "+".repeat(65) + ".";
        let (output, _, _) = run_jit(&src, &[]);
        assert_eq!(output, b"A");
    }

    #[test]
    fn jit_multi_cell() {
        let (_, ptr, mem) = run_jit(">+>++>+++", &[]);
        assert_eq!(ptr, 3);
        assert_eq!(mem[1], 1);
        assert_eq!(mem[2], 2);
        assert_eq!(mem[3], 3);
    }

    #[test]
    fn jit_simple_loop() {
        // [>+<-] with cell[0]=5 → cell[1]=5, cell[0]=0
        let (_, ptr, mem) = run_jit("+++++[>+<-]", &[]);
        assert_eq!(mem[0], 0);
        assert_eq!(mem[1], 5);
        assert_eq!(ptr, 0);
    }

    #[test]
    fn jit_loop_multiply() {
        // ++++[>++<-] → cell[1]=8
        let (_, _, mem) = run_jit("++++[>++<-]", &[]);
        assert_eq!(mem[0], 0);
        assert_eq!(mem[1], 8);
    }

    #[test]
    fn jit_nested_loops() {
        // ++[>++[>++<-]<-]
        // Outer: 2 iterations. Inner: 2 iterations each. Inner body: cell[2]+=2 each inner iter.
        // cell[2] = 2 outer * 2 inner * 2 = 8
        let (_, _, mem) = run_jit("++[>++[>++<-]<-]", &[]);
        assert_eq!(mem[0], 0);
        assert_eq!(mem[2], 8);
    }

    #[test]
    fn jit_input_callback() {
        let (output, _, _) = run_jit(",[.,]", b"AB");
        assert_eq!(output, b"AB");
    }
}
