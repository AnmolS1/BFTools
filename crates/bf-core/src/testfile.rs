use serde::Deserialize;
use std::path::{Path, PathBuf};

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
    /// Expected stdout output (trailing newline stripped).
    pub expected_output: Vec<u8>,
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

        let input = raw_case
            .input
            .map(|s| s.into_bytes())
            .unwrap_or_default();

        // Strip a single trailing newline from expected_output before storage.
        let mut expected = raw_case.expected_output.into_bytes();
        if expected.last() == Some(&b'\n') {
            expected.pop();
        }

        tests.push(TestCase {
            name: raw_case.name,
            program,
            input,
            expected_output: expected,
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
}
