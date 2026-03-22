use bf_core::testfile::parse_test_file;
use bf_testrunner::runner::{format_result, run_test};
use std::path::Path;
use std::process;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: bf-testrunner <file.bft>");
        process::exit(1);
    }

    let path = Path::new(&args[1]);
    let test_file = match parse_test_file(path) {
        Ok(tf) => tf,
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    };

    let mut any_failed = false;
    for case in &test_file.tests {
        let result = run_test(case);
        if !result.passed {
            any_failed = true;
        }
        println!("{}", format_result(&result));
    }

    if any_failed {
        process::exit(1);
    }
}
