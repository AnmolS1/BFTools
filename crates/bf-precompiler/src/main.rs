use bf_core::lexer::tokenize;
use bf_core::parser::parse;
use bf_precompiler::optimizer::optimize;
use std::path::PathBuf;
use std::process;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut input_path: Option<String> = None;
    let mut output_path: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                i += 1;
                if i < args.len() {
                    output_path = Some(args[i].clone());
                } else {
                    eprintln!("Error: -o requires an argument");
                    process::exit(1);
                }
            }
            arg => {
                if input_path.is_some() {
                    eprintln!("Error: multiple input files specified");
                    process::exit(1);
                }
                input_path = Some(arg.to_string());
            }
        }
        i += 1;
    }

    let input_path = match input_path {
        Some(p) => PathBuf::from(p),
        None => {
            eprintln!("Usage: bf-precompiler <input.bf> [-o output]");
            process::exit(1);
        }
    };

    let output_path = match output_path {
        Some(p) => PathBuf::from(p),
        None => input_path.with_extension(""),
    };

    let source = match std::fs::read_to_string(&input_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading '{}': {}", input_path.display(), e);
            process::exit(1);
        }
    };

    let tokens = tokenize(&source);
    let program = match parse(&tokens) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Syntax error: {}", e);
            process::exit(1);
        }
    };

    let ops = optimize(&program);

    #[cfg(feature = "aot")]
    {
        use bf_precompiler::compiler::{compile_to_object, link_executable};

        let obj_path = output_path.with_extension("o");

        if let Err(e) = compile_to_object(&ops, &obj_path) {
            eprintln!("Compilation error: {}", e);
            process::exit(1);
        }

        if let Err(e) = link_executable(&obj_path, &output_path) {
            eprintln!("Link error: {}", e);
            process::exit(1);
        }

        // Clean up the intermediate .o file.
        let _ = std::fs::remove_file(&obj_path);

        eprintln!("Compiled: {}", output_path.display());
    }

    #[cfg(not(feature = "aot"))]
    {
        let _ = ops;
        let _ = &output_path;
        eprintln!(
            "bf-precompiler: compiled with optimizer only (build with --features aot for native code output)"
        );
        eprintln!("Parsed and optimized {} (use --features aot to emit an executable)", input_path.display());
    }
}
