use bf_core::testfile::{DataFormat, TestCase};
use bf_core::lexer::tokenize;
use bf_core::parser::parse;
use bf_interpreter::Interpreter;

pub struct DiffInfo {
    pub byte_index: usize,
    /// None when actual output ended early (missing byte)
    pub expected_byte: Option<u8>,
    /// None when output is too long (extra byte beyond expected)
    pub actual_byte: Option<u8>,
}

pub struct TestResult {
    pub name: String,
    pub program: String,
    pub passed: bool,
    pub expected: Vec<u8>,
    pub actual: Vec<u8>,
    pub first_diff: Option<DiffInfo>,
    pub output_format: DataFormat,
}

/// Run a single test case and return its result.
pub fn run_test(case: &TestCase) -> TestResult {
    let program_name = case
        .program
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_else(|| case.program.display().to_string());

    let source = match std::fs::read_to_string(&case.program) {
        Ok(s) => s,
        Err(_e) => {
            return TestResult {
                name: case.name.clone(),
                program: program_name,
                passed: false,
                expected: case.expected_output.clone(),
                actual: Vec::new(),
                first_diff: Some(DiffInfo {
                    byte_index: 0,
                    expected_byte: case.expected_output.first().copied(),
                    actual_byte: None,
                }),
                output_format: case.output_format,
            };
        }
    };

    let tokens = tokenize(&source);
    let program = match parse(&tokens) {
        Ok(p) => p,
        Err(_) => {
            return TestResult {
                name: case.name.clone(),
                program: program_name,
                passed: false,
                expected: case.expected_output.clone(),
                actual: Vec::new(),
                first_diff: Some(DiffInfo {
                    byte_index: 0,
                    expected_byte: case.expected_output.first().copied(),
                    actual_byte: None,
                }),
                output_format: case.output_format,
            };
        }
    };

    let mut input = std::io::Cursor::new(case.input.clone());
    let mut output_buf = Vec::new();
    let mut interp = Interpreter::new(&program);
    let _ = interp.run(&mut input, &mut output_buf);

    // Strip trailing newline from actual output only for text format.
    let mut actual = output_buf;
    if case.output_format == DataFormat::Text {
        if actual.last() == Some(&b'\n') {
            actual.pop();
        }
    }

    let expected = &case.expected_output;

    // Find first difference.
    let first_diff = find_first_diff(expected, &actual);
    let passed = first_diff.is_none();

    TestResult {
        name: case.name.clone(),
        program: program_name,
        passed,
        expected: expected.clone(),
        actual,
        first_diff,
        output_format: case.output_format,
    }
}

fn find_first_diff(expected: &[u8], actual: &[u8]) -> Option<DiffInfo> {
    let len = expected.len().max(actual.len());
    for i in 0..len {
        match (expected.get(i), actual.get(i)) {
            (Some(&e), Some(&a)) if e == a => {}
            (Some(&e), Some(&a)) => {
                return Some(DiffInfo {
                    byte_index: i,
                    expected_byte: Some(e),
                    actual_byte: Some(a),
                });
            }
            (Some(&e), None) => {
                return Some(DiffInfo {
                    byte_index: i,
                    expected_byte: Some(e),
                    actual_byte: None,
                });
            }
            (None, Some(&a)) => {
                return Some(DiffInfo {
                    byte_index: i,
                    expected_byte: None,
                    actual_byte: Some(a),
                });
            }
            (None, None) => break,
        }
    }
    None
}

fn format_bytes(bytes: &[u8], fmt: DataFormat) -> String {
    match fmt {
        DataFormat::Text => String::from_utf8_lossy(bytes).into_owned(),
        DataFormat::Integers => bytes
            .iter()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// Format a test result as a human-readable string (PASS or FAIL with diff).
pub fn format_result(result: &TestResult) -> String {
    if result.passed {
        format!("PASS  {:<25}  ({})", result.name, result.program)
    } else {
        let mut out = format!("FAIL  {:<25}  ({})\n", result.name, result.program);
        out += &format!(
            "      Expected : {}\n",
            format_bytes(&result.expected, result.output_format)
        );
        out += &format!(
            "      Actual   : {}\n",
            format_bytes(&result.actual, result.output_format)
        );
        if let Some(diff) = &result.first_diff {
            match (diff.expected_byte, diff.actual_byte) {
                (Some(e), Some(a)) => {
                    out += &format!(
                        "      First diff at byte {}: expected {} (0x{:02X}), got {} (0x{:02X})",
                        diff.byte_index,
                        byte_repr(e),
                        e,
                        byte_repr(a),
                        a,
                    );
                }
                (Some(_), None) => {
                    out += &format!(
                        "      Output ended early at byte {}",
                        diff.byte_index
                    );
                }
                (None, Some(_)) => {
                    out += &format!(
                        "      Output too long (extra bytes starting at byte {})",
                        diff.byte_index
                    );
                }
                (None, None) => {}
            }
        }
        out
    }
}

fn byte_repr(b: u8) -> String {
    if (32..127).contains(&b) {
        format!("'{}'", b as char)
    } else {
        format!("0x{:02X}", b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bf_core::testfile::{DataFormat, TestCase};
    use std::path::PathBuf;

    fn fixture_path(name: &str) -> PathBuf {
        // test-fixtures/ is at workspace root, 3 levels up from crates/bf-testrunner/
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("../../test-fixtures");
        p.push(name);
        p
    }

    #[test]
    fn passing_test() {
        let case = TestCase {
            name: "Hello World".to_string(),
            program: fixture_path("hello_world.bf"),
            input: vec![],
            expected_output: b"Hello World!".to_vec(),
            output_format: DataFormat::Text,
        };
        let result = run_test(&case);
        assert!(result.passed, "expected pass but got: {}", format_result(&result));
    }

    #[test]
    fn failing_test_has_diff_info() {
        let case = TestCase {
            name: "Wrong Output".to_string(),
            program: fixture_path("hello_world.bf"),
            input: vec![],
            expected_output: b"Goodbye World!".to_vec(),
            output_format: DataFormat::Text,
        };
        let result = run_test(&case);
        assert!(!result.passed);
        assert!(result.first_diff.is_some());
        let diff = result.first_diff.unwrap();
        assert_eq!(diff.byte_index, 0); // 'H' vs 'G'
    }

    #[test]
    fn format_fail_contains_diff_position() {
        let case = TestCase {
            name: "Test".to_string(),
            program: fixture_path("hello_world.bf"),
            input: vec![],
            expected_output: b"Hello World?".to_vec(), // last char differs
            output_format: DataFormat::Text,
        };
        let result = run_test(&case);
        assert!(!result.passed);
        let text = format_result(&result);
        assert!(text.contains("FAIL"));
        assert!(text.contains("First diff at byte"));
    }

    #[test]
    fn integer_format_display() {
        // When output_format is Integers, format_result shows space-separated integers
        let case = TestCase {
            name: "IntDisplay".to_string(),
            program: fixture_path("hello_world.bf"),
            input: vec![],
            expected_output: vec![7u8],
            output_format: DataFormat::Integers,
        };
        let result = run_test(&case);
        assert!(!result.passed);
        let text = format_result(&result);
        // Expected shows "7" (the integer), not a garbled character
        assert!(text.contains("Expected : 7"));
    }
}
