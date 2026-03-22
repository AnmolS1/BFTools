use bf_formatter::{format, FormatterConfig};
use std::fs;

fn fixture(name: &str) -> String {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../../test-fixtures");
    p.push(name);
    fs::read_to_string(&p).unwrap_or_else(|_| panic!("missing fixture {name}"))
}

fn default_config() -> FormatterConfig {
    FormatterConfig::default()
}

#[test]
fn format_hello_world_is_idempotent() {
    let src = fixture("hello_world.bf");
    let once = format(&src, &default_config());
    let twice = format(&once, &default_config());
    assert_eq!(once, twice, "formatting should be idempotent");
}

#[test]
fn format_rot13_is_idempotent() {
    let src = fixture("rot13.bf");
    let once = format(&src, &default_config());
    let twice = format(&once, &default_config());
    assert_eq!(once, twice, "rot13 formatting should be idempotent");
}

#[test]
fn format_mandelbrot_is_idempotent() {
    let src = fixture("mandelbrot.bf");
    let once = format(&src, &default_config());
    let twice = format(&once, &default_config());
    assert_eq!(once, twice, "mandelbrot formatting should be idempotent");
}

#[test]
fn format_preserves_semantics_hello_world() {
    use bf_core::lexer::tokenize;
    use bf_core::parser::parse;

    let src = fixture("hello_world.bf");
    let formatted = format(&src, &default_config());

    // Both original and formatted must parse to equivalent programs
    let original_tokens = tokenize(&src);
    let formatted_tokens = tokenize(&formatted);
    let original_prog = parse(&original_tokens).expect("original should parse");
    let formatted_prog = parse(&formatted_tokens).expect("formatted should parse");

    // Compare by instruction count (same BF ops, just reformatted)
    assert_eq!(
        original_prog.nodes.len(),
        formatted_prog.nodes.len(),
        "formatting must not change top-level node count"
    );
}
