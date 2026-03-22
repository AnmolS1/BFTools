use bf_core::lexer::tokenize;
use bf_core::parser::parse;
use bf_interpreter::Interpreter;
use std::fs;

fn run_program(source: &str, input: &[u8]) -> Result<Vec<u8>, bf_interpreter::RuntimeError> {
    let tokens = tokenize(source);
    let program = parse(&tokens).expect("parse error");
    let mut interp = Interpreter::new(&program);
    let mut input_reader = std::io::Cursor::new(input.to_vec());
    let mut output = Vec::new();
    interp.run(&mut input_reader, &mut output)?;
    Ok(output)
}

fn fixture(name: &str) -> String {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../../test-fixtures");
    p.push(name);
    fs::read_to_string(&p).unwrap_or_else(|_| panic!("missing fixture {name}"))
}

#[test]
fn hello_world_exact_output() {
    let src = fixture("hello_world.bf");
    let output = run_program(&src, &[]).expect("runtime error");
    assert_eq!(output, b"Hello World!\n");
}

#[test]
fn cat_echo_single_char() {
    // `,.' — read one byte and write it
    let output = run_program(",.", b"A").expect("runtime error");
    assert_eq!(output, b"A");
}

#[test]
fn cell_wraps_255_to_0() {
    // Set cell to 255, then increment: should wrap to 0 (wrapping u8).
    // 255 '+' then '+' then '.': 256 → 0
    let src = "+".repeat(255) + "+.";
    let output = run_program(&src, &[]).expect("runtime error");
    assert_eq!(output, b"\x00");
}

#[test]
fn cell_wraps_0_to_255() {
    // Cell starts at 0; decrement once → 255 (wrapping_sub).
    let output = run_program("-.", &[]).expect("runtime error");
    assert_eq!(output, b"\xff");
}

#[test]
fn loop_skips_when_zero() {
    // `[+]` should not execute since cell starts at 0.
    // Then `.` outputs 0.
    let output = run_program("[+].", &[]).expect("runtime error");
    assert_eq!(output, b"\x00");
}

#[test]
fn simple_arithmetic_outputs_correct_byte() {
    // 8 × 8 = 64; '@ ' is 64
    let src = "++++++++[->++++++++<]>.";
    let output = run_program(src, &[]).expect("runtime error");
    assert_eq!(output, b"@");
}

#[test]
fn pointer_movement_accesses_different_cells() {
    // Set cell 0 = 1, cell 1 = 2; output both
    let output = run_program("+.>++.", &[]).expect("runtime error");
    assert_eq!(output, b"\x01\x02");
}

#[test]
#[ignore] // slow
fn mandelbrot_starts_correctly() {
    let src = fixture("mandelbrot.bf");
    let output = run_program(&src, &[]).expect("runtime error");
    // Just check it produces non-empty output starting with expected chars
    assert!(!output.is_empty(), "mandelbrot produced no output");
    // First line should be all spaces or '*' characters
    let first_newline = output.iter().position(|&b| b == b'\n').unwrap_or(output.len());
    let first_line = &output[..first_newline];
    assert!(
        first_line.iter().all(|&b| b == b' ' || b == b'*'),
        "unexpected chars in first line: {:?}",
        first_line
    );
}
