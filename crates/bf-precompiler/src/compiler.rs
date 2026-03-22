//! AOT compiler: `OptimizedOp` → Cranelift IR → native object file → linked executable.

use crate::optimizer::OptimizedOp;
use std::path::Path;
use std::process::Command;

use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::types::*;
use cranelift_codegen::ir::{AbiParam, Function, InstBuilder, MemFlags, UserFuncName};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_codegen::Context;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_module::{default_libcall_names, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};

const TAPE_SIZE: i64 = 30_000;

/// Compile `ops` to a native object file at `obj_path`.
pub fn compile_to_object(ops: &[OptimizedOp], obj_path: &Path) -> Result<(), String> {
    // Build ISA for the host target.
    let mut flag_builder = settings::builder();
    flag_builder
        .set("use_colocated_libcalls", "false")
        .map_err(|e| format!("flag error: {e}"))?;
    // Enable PIC on macOS/Linux since those platforms require it for shared libs
    // and the macOS linker requires PIC for text relocations to external symbols.
    let is_pic = if cfg!(target_os = "windows") { "false" } else { "true" };
    flag_builder
        .set("is_pic", is_pic)
        .map_err(|e| format!("flag error: {e}"))?;
    let flags = settings::Flags::new(flag_builder);

    let isa = cranelift_codegen::isa::lookup(target_lexicon::Triple::host())
        .map_err(|e| format!("ISA lookup: {e}"))?
        .finish(flags)
        .map_err(|e| format!("ISA finish: {e}"))?;

    let mut module = ObjectModule::new(
        ObjectBuilder::new(isa, "bf_aot", default_libcall_names())
            .map_err(|e| format!("ObjectBuilder: {e}"))?,
    );

    // Declare external libc functions.
    let putchar_id = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(I32));
        sig.returns.push(AbiParam::new(I32));
        module
            .declare_function("putchar", Linkage::Import, &sig)
            .map_err(|e| format!("declare putchar: {e}"))?
    };

    let getchar_id = {
        let mut sig = module.make_signature();
        sig.returns.push(AbiParam::new(I32));
        module
            .declare_function("getchar", Linkage::Import, &sig)
            .map_err(|e| format!("declare getchar: {e}"))?
    };

    // Declare the tape as a BSS data object (30,000 bytes, zero-initialized).
    let tape_id = {
        let mut data = cranelift_module::DataDescription::new();
        data.define_zeroinit(TAPE_SIZE as usize);
        module
            .declare_data("bf_tape", Linkage::Local, true, false)
            .and_then(|id| {
                module.define_data(id, &data)?;
                Ok(id)
            })
            .map_err(|e| format!("declare tape: {e}"))?
    };

    // Build main() function.
    let main_sig = {
        let mut sig = module.make_signature();
        // On most platforms, main returns i32.
        sig.returns.push(AbiParam::new(I32));
        sig
    };

    let main_id = module
        .declare_function("main", Linkage::Export, &main_sig)
        .map_err(|e| format!("declare main: {e}"))?;

    // Compile the function.
    let mut ctx = Context::new();
    ctx.func = Function::with_name_signature(
        UserFuncName::user(0, main_id.as_u32()),
        main_sig.clone(),
    );

    let mut func_ctx = FunctionBuilderContext::new();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);

        // Variables: ptr (pointer index, pointer-sized)
        let v_ptr = Variable::from_u32(0);
        builder.declare_var(v_ptr, module.target_config().pointer_type());

        let entry = builder.create_block();
        builder.seal_block(entry);
        builder.switch_to_block(entry);

        // Load tape base address from the global symbol.
        let tape_ref = module
            .declare_data_in_func(tape_id, builder.func);
        let ptr_ty = module.target_config().pointer_type();
        let tape_base = builder.ins().symbol_value(ptr_ty, tape_ref);

        // ptr = 0
        let zero = builder.ins().iconst(ptr_ty, 0);
        builder.def_var(v_ptr, zero);

        // Import putchar/getchar into this function.
        let putchar_ref = module.declare_func_in_func(putchar_id, builder.func);
        let getchar_ref = module.declare_func_in_func(getchar_id, builder.func);

        // Emit instructions.
        emit_ops(
            &mut builder,
            ops,
            tape_base,
            v_ptr,
            putchar_ref,
            getchar_ref,
            ptr_ty,
        );

        // return 0
        let ret_val = builder.ins().iconst(I32, 0);
        builder.ins().return_(&[ret_val]);

        builder.finalize();
    }

    module
        .define_function(main_id, &mut ctx)
        .map_err(|e| format!("define main: {e}"))?;

    let product = module.finish();
    let obj_bytes = product.emit().map_err(|e| format!("emit object: {e}"))?;

    std::fs::write(obj_path, &obj_bytes)
        .map_err(|e| format!("write object file: {e}"))?;

    Ok(())
}

/// Recursively emit Cranelift IR for a slice of `OptimizedOp`.
fn emit_ops(
    builder: &mut FunctionBuilder<'_>,
    ops: &[OptimizedOp],
    tape_base: cranelift_codegen::ir::Value,
    v_ptr: Variable,
    putchar_ref: cranelift_codegen::ir::FuncRef,
    getchar_ref: cranelift_codegen::ir::FuncRef,
    ptr_ty: cranelift_codegen::ir::Type,
) {
    let mf = MemFlags::trusted();
    for op in ops {
        match op {
            OptimizedOp::Add(n) => {
                let ptr = builder.use_var(v_ptr);
                let addr = builder.ins().iadd(tape_base, ptr);
                let val = builder.ins().load(I8, mf, addr, 0);
                let delta = builder.ins().iconst(I8, *n as i64);
                let new_val = builder.ins().iadd(val, delta);
                builder.ins().store(mf, new_val, addr, 0);
            }
            OptimizedOp::Sub(n) => {
                let ptr = builder.use_var(v_ptr);
                let addr = builder.ins().iadd(tape_base, ptr);
                let val = builder.ins().load(I8, mf, addr, 0);
                let delta = builder.ins().iconst(I8, *n as i64);
                let new_val = builder.ins().isub(val, delta);
                builder.ins().store(mf, new_val, addr, 0);
            }
            OptimizedOp::Right(n) => {
                let ptr = builder.use_var(v_ptr);
                let delta = builder.ins().iconst(ptr_ty, *n as i64);
                let new_ptr = builder.ins().iadd(ptr, delta);
                builder.def_var(v_ptr, new_ptr);
            }
            OptimizedOp::Left(n) => {
                let ptr = builder.use_var(v_ptr);
                let delta = builder.ins().iconst(ptr_ty, *n as i64);
                let new_ptr = builder.ins().isub(ptr, delta);
                builder.def_var(v_ptr, new_ptr);
            }
            OptimizedOp::Write => {
                let ptr = builder.use_var(v_ptr);
                let addr = builder.ins().iadd(tape_base, ptr);
                let val8 = builder.ins().load(I8, mf, addr, 0);
                let val32 = builder.ins().uextend(I32, val8);
                builder.ins().call(putchar_ref, &[val32]);
            }
            OptimizedOp::Read => {
                let ch = builder.ins().call(getchar_ref, &[]);
                let ch_val = builder.inst_results(ch)[0];
                // getchar returns -1 (EOF); clamp to 0 per BF spec.
                let zero32 = builder.ins().iconst(I32, 0);
                let is_eof = builder.ins().icmp(IntCC::SignedLessThan, ch_val, zero32);
                let clamped = builder.ins().select(is_eof, zero32, ch_val);
                let byte_val = builder.ins().ireduce(I8, clamped);
                let ptr = builder.use_var(v_ptr);
                let addr = builder.ins().iadd(tape_base, ptr);
                builder.ins().store(mf, byte_val, addr, 0);
            }
            OptimizedOp::ZeroCell => {
                let ptr = builder.use_var(v_ptr);
                let addr = builder.ins().iadd(tape_base, ptr);
                let zero = builder.ins().iconst(I8, 0);
                builder.ins().store(mf, zero, addr, 0);
            }
            OptimizedOp::MoveRight(offset) => {
                // cell[ptr + offset] += cell[ptr]; cell[ptr] = 0
                let ptr = builder.use_var(v_ptr);
                let src_addr = builder.ins().iadd(tape_base, ptr);
                let src_val = builder.ins().load(I8, mf, src_addr, 0);
                let off = builder.ins().iconst(ptr_ty, *offset as i64);
                let dst_ptr = builder.ins().iadd(ptr, off);
                let dst_addr = builder.ins().iadd(tape_base, dst_ptr);
                let dst_val = builder.ins().load(I8, mf, dst_addr, 0);
                let new_dst = builder.ins().iadd(dst_val, src_val);
                builder.ins().store(mf, new_dst, dst_addr, 0);
                let zero = builder.ins().iconst(I8, 0);
                builder.ins().store(mf, zero, src_addr, 0);
            }
            OptimizedOp::MoveLeft(offset) => {
                let ptr = builder.use_var(v_ptr);
                let src_addr = builder.ins().iadd(tape_base, ptr);
                let src_val = builder.ins().load(I8, mf, src_addr, 0);
                let off = builder.ins().iconst(ptr_ty, *offset as i64);
                let dst_ptr = builder.ins().isub(ptr, off);
                let dst_addr = builder.ins().iadd(tape_base, dst_ptr);
                let dst_val = builder.ins().load(I8, mf, dst_addr, 0);
                let new_dst = builder.ins().iadd(dst_val, src_val);
                builder.ins().store(mf, new_dst, dst_addr, 0);
                let zero = builder.ins().iconst(I8, 0);
                builder.ins().store(mf, zero, src_addr, 0);
            }
            OptimizedOp::MultiplyRight(offset, factor) => {
                let ptr = builder.use_var(v_ptr);
                let src_addr = builder.ins().iadd(tape_base, ptr);
                let src_val = builder.ins().load(I8, mf, src_addr, 0);
                let fact = builder.ins().iconst(I8, *factor as i64);
                let product = builder.ins().imul(src_val, fact);
                let off = builder.ins().iconst(ptr_ty, *offset as i64);
                let dst_ptr = builder.ins().iadd(ptr, off);
                let dst_addr = builder.ins().iadd(tape_base, dst_ptr);
                let dst_val = builder.ins().load(I8, mf, dst_addr, 0);
                let new_dst = builder.ins().iadd(dst_val, product);
                builder.ins().store(mf, new_dst, dst_addr, 0);
                let zero = builder.ins().iconst(I8, 0);
                builder.ins().store(mf, zero, src_addr, 0);
            }
            OptimizedOp::FindZeroRight => {
                // while cell[ptr] != 0 { ptr++ }
                let cond_block = builder.create_block();
                let body_block = builder.create_block();
                let exit_block = builder.create_block();
                builder.ins().jump(cond_block, &[]);

                builder.switch_to_block(cond_block);
                let ptr = builder.use_var(v_ptr);
                let addr = builder.ins().iadd(tape_base, ptr);
                let val = builder.ins().load(I8, mf, addr, 0);
                let zero8 = builder.ins().iconst(I8, 0);
                let is_zero = builder.ins().icmp(IntCC::Equal, val, zero8);
                builder.ins().brif(is_zero, exit_block, &[], body_block, &[]);
                builder.seal_block(body_block);
                builder.seal_block(exit_block);

                builder.switch_to_block(body_block);
                let ptr2 = builder.use_var(v_ptr);
                let one = builder.ins().iconst(ptr_ty, 1);
                let new_ptr = builder.ins().iadd(ptr2, one);
                builder.def_var(v_ptr, new_ptr);
                builder.ins().jump(cond_block, &[]);
                builder.seal_block(cond_block);

                builder.switch_to_block(exit_block);
            }
            OptimizedOp::FindZeroLeft => {
                let cond_block = builder.create_block();
                let body_block = builder.create_block();
                let exit_block = builder.create_block();
                builder.ins().jump(cond_block, &[]);

                builder.switch_to_block(cond_block);
                let ptr = builder.use_var(v_ptr);
                let addr = builder.ins().iadd(tape_base, ptr);
                let val = builder.ins().load(I8, mf, addr, 0);
                let zero8 = builder.ins().iconst(I8, 0);
                let is_zero = builder.ins().icmp(IntCC::Equal, val, zero8);
                builder.ins().brif(is_zero, exit_block, &[], body_block, &[]);
                builder.seal_block(body_block);
                builder.seal_block(exit_block);

                builder.switch_to_block(body_block);
                let ptr2 = builder.use_var(v_ptr);
                let one = builder.ins().iconst(ptr_ty, 1);
                let new_ptr = builder.ins().isub(ptr2, one);
                builder.def_var(v_ptr, new_ptr);
                builder.ins().jump(cond_block, &[]);
                builder.seal_block(cond_block);

                builder.switch_to_block(exit_block);
            }
            OptimizedOp::Loop(body) => {
                let cond_block = builder.create_block();
                let body_block = builder.create_block();
                let exit_block = builder.create_block();
                builder.ins().jump(cond_block, &[]);

                builder.switch_to_block(cond_block);
                let ptr = builder.use_var(v_ptr);
                let addr = builder.ins().iadd(tape_base, ptr);
                let val = builder.ins().load(I8, mf, addr, 0);
                let zero8 = builder.ins().iconst(I8, 0);
                let is_zero = builder.ins().icmp(IntCC::Equal, val, zero8);
                builder.ins().brif(is_zero, exit_block, &[], body_block, &[]);
                builder.seal_block(body_block);
                builder.seal_block(exit_block);

                builder.switch_to_block(body_block);
                emit_ops(builder, body, tape_base, v_ptr, putchar_ref, getchar_ref, ptr_ty);
                builder.ins().jump(cond_block, &[]);
                builder.seal_block(cond_block);

                builder.switch_to_block(exit_block);
            }
        }
    }
}

/// Link an object file to a native executable using the system C compiler.
pub fn link_executable(obj_path: &Path, out_path: &Path) -> Result<(), String> {
    // Try `cc` first (Linux/macOS), fall back to `gcc`.
    let linker = if cfg!(target_os = "windows") {
        "link.exe"
    } else {
        "cc"
    };

    let status = if cfg!(target_os = "windows") {
        Command::new(linker)
            .arg(obj_path)
            .arg(format!("/OUT:{}", out_path.display()))
            .status()
    } else {
        Command::new(linker)
            .arg(obj_path)
            .arg("-o")
            .arg(out_path)
            .status()
    };

    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("linker exited with status {s}")),
        Err(e) => Err(format!("failed to run linker '{linker}': {e}")),
    }
}
