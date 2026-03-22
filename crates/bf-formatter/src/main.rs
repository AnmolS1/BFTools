use std::env;
use std::fs;
use std::process;

use bf_formatter::{format, FormatterConfig};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: bf-formatter <file.bf>");
        process::exit(1);
    }

    let path = &args[1];
    let input = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading {}: {}", path, e);
            process::exit(1);
        }
    };

    let config = FormatterConfig::default();
    let output = format(&input, &config);
    print!("{}", output);
}
