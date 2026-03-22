use bf_interpreter::Interpreter;

pub const SCOPE_CURRENT_CELL: u64 = 1;
pub const SCOPE_MEMORY: u64 = 2;
pub const SCOPE_TEST_MISMATCH: u64 = 3;

pub struct Scope {
    pub name: String,
    pub variables_reference: u64,
    pub expensive: bool,
}

pub struct Variable {
    pub name: String,
    pub value: String,
    pub variables_reference: u64,
}

/// Information about a test output mismatch, for testDebug mode.
pub struct MismatchInfo {
    pub diverged_at_byte: usize,
    pub expected_byte: Option<u8>,
    pub actual_byte: Option<u8>,
    pub output_so_far: Vec<u8>,
    pub expected_output: Vec<u8>,
}

pub fn get_scopes(mismatch: Option<&MismatchInfo>) -> Vec<Scope> {
    let mut scopes = vec![
        Scope {
            name: "Current Cell".to_string(),
            variables_reference: SCOPE_CURRENT_CELL,
            expensive: false,
        },
        Scope {
            name: "Memory".to_string(),
            variables_reference: SCOPE_MEMORY,
            expensive: false,
        },
    ];
    if mismatch.is_some() {
        scopes.push(Scope {
            name: "Test Mismatch".to_string(),
            variables_reference: SCOPE_TEST_MISMATCH,
            expensive: false,
        });
    }
    scopes
}

pub fn get_variables(
    scope_ref: u64,
    interp: &Interpreter,
    mismatch: Option<&MismatchInfo>,
) -> Vec<Variable> {
    match scope_ref {
        SCOPE_CURRENT_CELL => {
            let ptr = interp.get_pointer();
            let val = interp.get_memory()[ptr];
            vec![
                Variable {
                    name: "pointer".to_string(),
                    value: ptr.to_string(),
                    variables_reference: 0,
                },
                Variable {
                    name: "value".to_string(),
                    value: format!("{} (0x{:02X}) '{}'", val, val, printable(val)),
                    variables_reference: 0,
                },
            ]
        }
        SCOPE_MEMORY => {
            let mem = interp.get_memory();
            let last_nonzero =
                mem.iter().rposition(|&b| b != 0).map(|i| i + 1).unwrap_or(0);
            let display_len = std::cmp::max(last_nonzero, 10).min(mem.len());
            (0..display_len)
                .map(|i| Variable {
                    name: format!("[{}]", i),
                    value: mem[i].to_string(),
                    variables_reference: 0,
                })
                .collect()
        }
        SCOPE_TEST_MISMATCH => {
            if let Some(m) = mismatch {
                vec![
                    Variable {
                        name: "diverged_at_byte".to_string(),
                        value: m.diverged_at_byte.to_string(),
                        variables_reference: 0,
                    },
                    Variable {
                        name: "expected_byte".to_string(),
                        value: m
                            .expected_byte
                            .map(|b| format!("{} (0x{:02X})", printable_str(b), b))
                            .unwrap_or_else(|| "(end of expected output)".to_string()),
                        variables_reference: 0,
                    },
                    Variable {
                        name: "actual_byte".to_string(),
                        value: m
                            .actual_byte
                            .map(|b| format!("{} (0x{:02X})", printable_str(b), b))
                            .unwrap_or_else(|| "(program ended early)".to_string()),
                        variables_reference: 0,
                    },
                    Variable {
                        name: "output_so_far".to_string(),
                        value: String::from_utf8_lossy(&m.output_so_far).into_owned(),
                        variables_reference: 0,
                    },
                    Variable {
                        name: "expected_output".to_string(),
                        value: String::from_utf8_lossy(&m.expected_output).into_owned(),
                        variables_reference: 0,
                    },
                ]
            } else {
                vec![]
            }
        }
        _ => vec![],
    }
}

fn printable(b: u8) -> char {
    if (32..127).contains(&b) { b as char } else { '.' }
}

fn printable_str(b: u8) -> String {
    if (32..127).contains(&b) {
        format!("'{}'", b as char)
    } else {
        format!("0x{:02X}", b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bf_core::lexer::tokenize;
    use bf_core::parser::parse;

    fn interpreter_after(src: &str) -> Interpreter {
        let tokens = tokenize(src);
        let program = parse(&tokens).unwrap();
        let mut interp = Interpreter::new(&program);
        interp.run(&mut std::io::empty(), &mut std::io::sink()).unwrap();
        interp
    }

    #[test]
    fn current_cell_after_move_and_increment() {
        let interp = interpreter_after(">++");
        let vars = get_variables(SCOPE_CURRENT_CELL, &interp, None);
        assert_eq!(vars[0].value, "1");
        assert_eq!(&vars[1].value[..1], "2");
    }

    #[test]
    fn memory_scope_shows_at_least_ten_cells() {
        let interp = interpreter_after("");
        let vars = get_variables(SCOPE_MEMORY, &interp, None);
        assert_eq!(vars.len(), 10);
    }

    #[test]
    fn memory_scope_extends_to_last_nonzero() {
        let interp = interpreter_after(">>>>>+");
        let vars = get_variables(SCOPE_MEMORY, &interp, None);
        assert!(vars.len() >= 10);
        assert_eq!(vars[5].value, "1");
    }

    #[test]
    fn test_mismatch_scope_variables() {
        let interp = interpreter_after("");
        let m = MismatchInfo {
            diverged_at_byte: 2,
            expected_byte: Some(b'5'),
            actual_byte: Some(b'\n'),
            output_so_far: b"12".to_vec(),
            expected_output: b"125".to_vec(),
        };
        let vars = get_variables(SCOPE_TEST_MISMATCH, &interp, Some(&m));
        assert_eq!(vars[0].name, "diverged_at_byte");
        assert_eq!(vars[0].value, "2");
        assert!(vars[1].value.contains("'5'"));
        assert!(vars[2].value.contains("0x0A"));
    }
}
