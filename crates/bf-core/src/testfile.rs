use serde::Deserialize;
use std::path::{Path, PathBuf};

/// How a string value in a `.bft` field is interpreted as bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataFormat {
    /// Raw text: the string's UTF-8 bytes are used directly (default).
    #[default]
    Text,
    /// Space- or comma-separated decimal integers in 0–255, each becoming one byte.
    Integers,
}

#[derive(Debug, Deserialize)]
struct RawTestFile {
    test: Vec<RawTestCase>,
}

#[derive(Debug, Deserialize)]
struct RawTestCase {
    name: String,
    program: String,
    input: Option<String>,
    expected_output: String,
    input_format: Option<DataFormat>,
    output_format: Option<DataFormat>,
}

#[derive(Debug)]
pub struct TestFile {
    pub tests: Vec<TestCase>,
}

#[derive(Debug)]
pub struct TestCase {
    pub name: String,
    /// Absolute path to the .bf file.
    pub program: PathBuf,
    /// Bytes fed to the program's stdin.
    pub input: Vec<u8>,
    /// Expected stdout output (trailing newline stripped for Text format).
    pub expected_output: Vec<u8>,
    /// How `expected_output` should be displayed in test results.
    pub output_format: DataFormat,
}

/// Parse a space- or comma-separated list of decimal integers into bytes.
fn parse_integer_bytes(s: &str, field: &str, test_name: &str) -> Result<Vec<u8>, String> {
    s.split(|c: char| c == ',' || c.is_ascii_whitespace())
        .filter(|t| !t.is_empty())
        .map(|t| {
            t.parse::<u8>().map_err(|e| {
                format!(
                    "Test '{}': invalid byte value '{}' in {}: {}",
                    test_name, t, field, e
                )
            })
        })
        .collect()
}

/// Parse a `.bft` test file at `path`.
///
/// All `program` paths in the file are resolved relative to the `.bft` file's directory.
/// Returns an error if the TOML is malformed, required fields are missing, or a referenced
/// `.bf` file does not exist.
pub fn parse_test_file(path: &Path) -> Result<TestFile, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Cannot read {}: {}", path.display(), e))?;

    let raw: RawTestFile = toml::from_str(&content)
        .map_err(|e| format!("TOML parse error in {}: {}", path.display(), e))?;

    let dir = path.parent().unwrap_or(Path::new("."));

    let mut tests = Vec::with_capacity(raw.test.len());
    for raw_case in raw.test {
        let program = dir.join(&raw_case.program);
        if !program.exists() {
            return Err(format!(
                "Test '{}': program file '{}' does not exist",
                raw_case.name,
                program.display()
            ));
        }

        let in_fmt = raw_case.input_format.unwrap_or_default();
        let out_fmt = raw_case.output_format.unwrap_or_default();

        let input = match raw_case.input {
            Some(s) => match in_fmt {
                DataFormat::Text => s.into_bytes(),
                DataFormat::Integers => {
                    parse_integer_bytes(&s, "input", &raw_case.name)?
                }
            },
            None => Vec::new(),
        };

        let mut expected = match out_fmt {
            DataFormat::Text => raw_case.expected_output.into_bytes(),
            DataFormat::Integers => {
                parse_integer_bytes(&raw_case.expected_output, "expected_output", &raw_case.name)?
            }
        };

        // Strip a single trailing newline only for text format.
        if out_fmt == DataFormat::Text {
            if expected.last() == Some(&b'\n') {
                expected.pop();
            }
        }

        tests.push(TestCase {
            name: raw_case.name,
            program,
            input,
            expected_output: expected,
            output_format: out_fmt,
        });
    }

    Ok(TestFile { tests })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn tmp_dir() -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("bf_core_test_{n}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_file(dir: &Path, name: &str, content: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn valid_multi_test_file() {
        let dir = tmp_dir();
        write_file(&dir, "prog.bf", "+++.");
        let bft_path = write_file(
            &dir,
            "tests.bft",
            "[[test]]\nname = \"Test One\"\nprogram = \"prog.bf\"\nexpected_output = \"hello\\n\"\n\
             [[test]]\nname = \"Test Two\"\nprogram = \"prog.bf\"\ninput = \"abc\"\nexpected_output = \"world\"\n",
        );
        let tf = parse_test_file(&bft_path).unwrap();
        assert_eq!(tf.tests.len(), 2);
        assert_eq!(tf.tests[0].name, "Test One");
        assert_eq!(tf.tests[0].expected_output, b"hello"); // trailing \n stripped
        assert_eq!(tf.tests[1].input, b"abc");
        assert_eq!(tf.tests[1].expected_output, b"world");
    }

    #[test]
    fn missing_required_field_is_error() {
        let dir = tmp_dir();
        // Missing `expected_output`
        let bft_path = write_file(
            &dir,
            "bad.bft",
            "[[test]]\nname = \"No output field\"\nprogram = \"prog.bf\"\n",
        );
        assert!(parse_test_file(&bft_path).is_err());
    }

    #[test]
    fn nonexistent_bf_file_is_error() {
        let dir = tmp_dir();
        let bft_path = write_file(
            &dir,
            "tests.bft",
            "[[test]]\nname = \"Missing\"\nprogram = \"does_not_exist.bf\"\nexpected_output = \"x\"\n",
        );
        let result = parse_test_file(&bft_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    #[test]
    fn input_escape_sequences() {
        // TOML supports \uXXXX (4-hex-digit) unicode escapes in basic strings.
        let dir = tmp_dir();
        write_file(&dir, "prog.bf", ".");
        let bft_path = write_file(
            &dir,
            "tests.bft",
            "[[test]]\nname = \"Escape\"\nprogram = \"prog.bf\"\ninput = \"\\u0005\"\nexpected_output = \"x\"\n",
        );
        let tf = parse_test_file(&bft_path).unwrap();
        assert_eq!(tf.tests[0].input, vec![5u8]);
    }

    #[test]
    fn integer_output_format() {
        let dir = tmp_dir();
        write_file(&dir, "prog.bf", ".");
        let bft_path = write_file(
            &dir,
            "tests.bft",
            "[[test]]\nname = \"IntOut\"\nprogram = \"prog.bf\"\nexpected_output = \"72 101 108\"\noutput_format = \"integers\"\n",
        );
        let tf = parse_test_file(&bft_path).unwrap();
        assert_eq!(tf.tests[0].expected_output, vec![72u8, 101, 108]);
        assert_eq!(tf.tests[0].output_format, DataFormat::Integers);
    }

    #[test]
    fn integer_input_format() {
        let dir = tmp_dir();
        write_file(&dir, "prog.bf", ".");
        let bft_path = write_file(
            &dir,
            "tests.bft",
            "[[test]]\nname = \"IntIn\"\nprogram = \"prog.bf\"\ninput = \"5, 10\"\ninput_format = \"integers\"\nexpected_output = \"x\"\n",
        );
        let tf = parse_test_file(&bft_path).unwrap();
        assert_eq!(tf.tests[0].input, vec![5u8, 10]);
    }

    #[test]
    fn integer_format_no_newline_strip() {
        // Trailing-newline stripping must NOT apply to integer format.
        // "10" means byte 10 (newline), NOT "10\n" with the newline removed.
        let dir = tmp_dir();
        write_file(&dir, "prog.bf", ".");
        let bft_path = write_file(
            &dir,
            "tests.bft",
            "[[test]]\nname = \"NoStrip\"\nprogram = \"prog.bf\"\nexpected_output = \"10\"\noutput_format = \"integers\"\n",
        );
        let tf = parse_test_file(&bft_path).unwrap();
        assert_eq!(tf.tests[0].expected_output, vec![10u8]);
    }

    #[test]
    fn invalid_integer_value_is_error() {
        let dir = tmp_dir();
        write_file(&dir, "prog.bf", ".");
        let bft_path = write_file(
            &dir,
            "tests.bft",
            "[[test]]\nname = \"Bad\"\nprogram = \"prog.bf\"\nexpected_output = \"256\"\noutput_format = \"integers\"\n",
        );
        let result = parse_test_file(&bft_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("256"));
    }

    #[test]
    fn default_format_is_text() {
        let dir = tmp_dir();
        write_file(&dir, "prog.bf", ".");
        let bft_path = write_file(
            &dir,
            "tests.bft",
            "[[test]]\nname = \"Default\"\nprogram = \"prog.bf\"\nexpected_output = \"hello\"\n",
        );
        let tf = parse_test_file(&bft_path).unwrap();
        assert_eq!(tf.tests[0].output_format, DataFormat::Text);
        assert_eq!(tf.tests[0].expected_output, b"hello");
    }

    #[test]
    fn integer_single_value() {
        // The user's exact use case: expected_output = "7" with output_format = "integers"
        let dir = tmp_dir();
        write_file(&dir, "prog.bf", ".");
        let bft_path = write_file(
            &dir,
            "tests.bft",
            "[[test]]\nname = \"SingleInt\"\nprogram = \"prog.bf\"\nexpected_output = \"7\"\noutput_format = \"integers\"\n",
        );
        let tf = parse_test_file(&bft_path).unwrap();
        assert_eq!(tf.tests[0].expected_output, vec![7u8]); // 0x07, NOT b'7' (0x37)
    }
}
