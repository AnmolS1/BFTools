use bf_core::testfile::{parse_test_file, TestCase};
use bf_testrunner::runner::{format_result, run_test};
use std::path::PathBuf;

fn fixture_path(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../../test-fixtures");
    p.push(name);
    p
}

// ── parse_test_file + run_test via hello_world.bft ───────────────────────────

#[test]
fn hello_world_bft_all_pass() {
    let bft_path = fixture_path("hello_world.bft");
    let test_file = parse_test_file(&bft_path).expect("should parse hello_world.bft");
    assert!(!test_file.tests.is_empty(), "expected at least one test case");

    for case in &test_file.tests {
        let result = run_test(case);
        assert!(
            result.passed,
            "test '{}' should pass:\n{}",
            result.name,
            format_result(&result)
        );
    }
}

#[test]
fn pass_format_contains_pass_keyword() {
    let case = TestCase {
        name: "Hello World".to_string(),
        program: fixture_path("hello_world.bf"),
        input: vec![],
        expected_output: b"Hello World!".to_vec(),
    };
    let result = run_test(&case);
    assert!(result.passed);
    let text = format_result(&result);
    assert!(text.starts_with("PASS"), "expected PASS prefix, got: {text}");
}

#[test]
fn fail_format_contains_fail_and_diff_info() {
    let case = TestCase {
        name: "Wrong".to_string(),
        program: fixture_path("hello_world.bf"),
        input: vec![],
        expected_output: b"Goodbye World!".to_vec(),
    };
    let result = run_test(&case);
    assert!(!result.passed);
    let text = format_result(&result);
    assert!(text.starts_with("FAIL"), "expected FAIL prefix, got: {text}");
    assert!(
        text.contains("First diff at byte"),
        "expected diff info, got: {text}"
    );
}

#[test]
fn fail_reports_first_diff_byte_index() {
    // hello_world.bf outputs "Hello World!\n"; mismatch at position 6 (space vs '_')
    let mut expected = b"Hello _orld!".to_vec(); // differs at index 6
    let _ = expected; // suppress warning; re-derive below
    let expected_out = {
        let mut v = b"Hello World!".to_vec();
        v[6] = b'_'; // 'W' → '_'
        v
    };
    let case = TestCase {
        name: "Partial mismatch".to_string(),
        program: fixture_path("hello_world.bf"),
        input: vec![],
        expected_output: expected_out,
    };
    let result = run_test(&case);
    assert!(!result.passed);
    let diff = result.first_diff.expect("should have diff info");
    assert_eq!(diff.byte_index, 6, "mismatch should be at byte 6");
}

#[test]
fn missing_program_file_is_fail() {
    let case = TestCase {
        name: "Nonexistent".to_string(),
        program: fixture_path("does_not_exist.bf"),
        input: vec![],
        expected_output: b"anything".to_vec(),
    };
    let result = run_test(&case);
    assert!(!result.passed, "missing program should fail");
}
