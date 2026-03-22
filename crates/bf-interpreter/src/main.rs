use std::env;
use std::fs;
use std::io::{stdin, stdout};
use std::process;

use bf_core::lexer::tokenize;
use bf_core::parser::parse;
use bf_interpreter::Interpreter;

#[derive(Default)]
enum RunMode {
    #[default]
    Interpreter,
    Jit,
    AutoJit,
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut file_path: Option<String> = None;
    let mut mode = RunMode::Interpreter;

    for arg in &args[1..] {
        match arg.as_str() {
            "--jit" => mode = RunMode::Jit,
            "--auto-jit" => mode = RunMode::AutoJit,
            "--no-jit" => mode = RunMode::Interpreter,
            path => {
                if file_path.is_some() {
                    eprintln!("Error: multiple file arguments");
                    process::exit(1);
                }
                file_path = Some(path.to_string());
            }
        }
    }

    let file_path = match file_path {
        Some(p) => p,
        None => {
            eprintln!("Usage: bf-interpreter [--jit] [--no-jit] [--auto-jit] <file.bf>");
            process::exit(1);
        }
    };

    let source = match fs::read_to_string(&file_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading '{}': {}", file_path, e);
            process::exit(1);
        }
    };

    let tokens = tokenize(&source);
    let program = match parse(&tokens) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Syntax error in '{}': {}", file_path, e);
            process::exit(1);
        }
    };

    match mode {
        RunMode::Interpreter => {
            let mut interp = Interpreter::new(&program);
            match interp.run(&mut stdin().lock(), &mut stdout().lock()) {
                Ok(()) => {}
                Err(e) => {
                    eprintln!("Runtime error: {}", e);
                    process::exit(2);
                }
            }
        }
        RunMode::Jit => {
            #[cfg(feature = "jit")]
            {
                use bf_interpreter::jit::{JitCompiler, JitIoContext};
                use bf_interpreter::compile;

                let instructions = compile(&program);
                let mut jit = JitCompiler::new().unwrap_or_else(|e| {
                    eprintln!("JIT init error: {e}");
                    process::exit(1);
                });

                extern "C" fn write_byte(_ctx: *mut JitIoContext, byte: u8) {
                    use std::io::Write;
                    let _ = std::io::stdout().write_all(&[byte]);
                }
                extern "C" fn read_byte(_ctx: *mut JitIoContext) -> u8 {
                    use std::io::Read;
                    let mut buf = [0u8; 1];
                    match std::io::stdin().read(&mut buf) {
                        Ok(1) => buf[0],
                        _ => 0,
                    }
                }

                let mut ctx = JitIoContext {
                    write_byte,
                    read_byte,
                };

                match jit.compile(&instructions) {
                    Ok(f) => {
                        let mut tape = vec![0u8; 30_000];
                        // SAFETY: compiled code correctly accesses only the tape slice
                        unsafe { f(tape.as_mut_ptr(), tape.len(), &mut ctx) };
                    }
                    Err(e) => {
                        eprintln!("JIT compile error: {e}");
                        process::exit(1);
                    }
                }
            }
            #[cfg(not(feature = "jit"))]
            {
                eprintln!("JIT not available (build with --features jit). Running interpreter.");
                let mut interp = Interpreter::new(&program);
                match interp.run(&mut stdin().lock(), &mut stdout().lock()) {
                    Ok(()) => {}
                    Err(e) => { eprintln!("Runtime error: {}", e); process::exit(2); }
                }
            }
        }
        RunMode::AutoJit => {
            #[cfg(feature = "jit")]
            {
                use bf_interpreter::interpreter::auto_jit::AutoJitInterpreter;
                let mut interp = AutoJitInterpreter::new(&program).unwrap_or_else(|e| {
                    eprintln!("Auto-JIT init error: {e}");
                    process::exit(1);
                });
                match interp.run(&mut stdin().lock(), &mut stdout().lock()) {
                    Ok(()) => {}
                    Err(e) => {
                        eprintln!("Runtime error: {}", e);
                        process::exit(2);
                    }
                }
            }
            #[cfg(not(feature = "jit"))]
            {
                eprintln!("Auto-JIT not available (build with --features jit). Running interpreter.");
                let mut interp = Interpreter::new(&program);
                match interp.run(&mut stdin().lock(), &mut stdout().lock()) {
                    Ok(()) => {}
                    Err(e) => { eprintln!("Runtime error: {}", e); process::exit(2); }
                }
            }
        }
    }
}
