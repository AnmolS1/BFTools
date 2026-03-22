use std::collections::HashMap;

use bf_core::lexer::tokenize;
use bf_core::parser::parse;
use bf_interpreter::{compile, Interpreter, Opcode, StepResult};
use serde_json::{json, Value};

use crate::breakpoints::{build_source_map, BreakpointManager};
use crate::debugger::{
    get_scopes, get_variables, MismatchInfo, SCOPE_CURRENT_CELL, SCOPE_MEMORY,
    SCOPE_TEST_MISMATCH,
};
use crate::transport::DapTransport;

pub struct DapAdapter {
    transport: DapTransport,
    seq: u64,
    interpreter: Option<Interpreter>,
    source_map: HashMap<usize, Vec<usize>>,
    source_file: String,
    breakpoints: BreakpointManager,
    finished: bool,
    // testDebug mode
    test_mode: bool,
    test_input: Vec<u8>,
    test_expected: Vec<u8>,
    test_output_counter: usize,
    test_output_so_far: Vec<u8>,
    test_mismatch: Option<MismatchInfo>,
}

impl DapAdapter {
    pub fn new(transport: DapTransport) -> Self {
        DapAdapter {
            transport,
            seq: 1,
            interpreter: None,
            source_map: HashMap::new(),
            source_file: String::new(),
            breakpoints: BreakpointManager::new(),
            finished: false,
            test_mode: false,
            test_input: Vec::new(),
            test_expected: Vec::new(),
            test_output_counter: 0,
            test_output_so_far: Vec::new(),
            test_mismatch: None,
        }
    }

    pub fn run(&mut self) {
        while let Some(msg) = self.transport.read_message() {
            if msg.get("type").and_then(|t| t.as_str()) == Some("request") {
                self.handle_request(msg);
            }
        }
    }

    fn handle_request(&mut self, msg: Value) {
        let seq = msg["seq"].as_u64().unwrap_or(0);
        let cmd = msg["command"].as_str().unwrap_or("").to_string();
        let args = msg.get("arguments").cloned().unwrap_or(json!({}));

        match cmd.as_str() {
            "initialize" => self.handle_initialize(seq),
            "launch" => self.handle_launch(seq, &args),
            "configurationDone" => self.handle_configuration_done(seq),
            "setBreakpoints" => self.handle_set_breakpoints(seq, &args),
            "threads" => self.handle_threads(seq),
            "stackTrace" => self.handle_stack_trace(seq),
            "scopes" => self.handle_scopes(seq),
            "variables" => self.handle_variables(seq, &args),
            "continue" => self.handle_continue(seq),
            "next" | "stepIn" => self.handle_next(seq),
            "stepOut" => self.handle_step_out(seq),
            "pause" => self.send_response(seq, &cmd, true, json!({})),
            "terminate" | "disconnect" => {
                self.send_response(seq, &cmd, true, json!({}));
                self.send_event("terminated", json!({}));
            }
            other => {
                self.send_response(seq, other, false, json!({"error": "unsupported command"}));
            }
        }
    }

    fn handle_initialize(&mut self, seq: u64) {
        self.send_response(
            seq,
            "initialize",
            true,
            json!({
                "supportsConfigurationDoneRequest": true,
                "supportsSetBreakpointsRequest": true,
                "supportsStepBack": false,
                "supportsRestartRequest": false,
                "supportsTerminateRequest": true,
            }),
        );
        self.send_event("initialized", json!({}));
    }

    fn handle_launch(&mut self, seq: u64, args: &Value) {
        // Detect testDebug vs standard launch.
        let request_type = args
            .get("request")
            .and_then(|v| v.as_str())
            .unwrap_or("launch");

        if request_type == "testDebug" {
            self.setup_test_debug(seq, args);
            return;
        }

        // Standard launch.
        let source = if let Some(text) = args.get("programText").and_then(|v| v.as_str()) {
            text.to_string()
        } else if let Some(path) = args.get("program").and_then(|v| v.as_str()) {
            self.source_file = path.to_string();
            match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(e) => {
                    self.send_response(seq, "launch", false, json!({"error": e.to_string()}));
                    return;
                }
            }
        } else {
            self.send_response(seq, "launch", false, json!({"error": "no program specified"}));
            return;
        };

        let tokens = tokenize(&source);
        let program = match parse(&tokens) {
            Ok(p) => p,
            Err(e) => {
                self.send_response(seq, "launch", false, json!({"error": format!("{e}")}));
                return;
            }
        };

        let instructions = compile(&program);
        self.source_map = build_source_map(&instructions);
        self.interpreter = Some(Interpreter::new(&program));
        self.finished = false;
        self.test_mode = false;

        self.send_response(seq, "launch", true, json!({}));
    }

    fn setup_test_debug(&mut self, seq: u64, args: &Value) {
        let source = if let Some(text) = args.get("programText").and_then(|v| v.as_str()) {
            text.to_string()
        } else {
            let path = match args.get("program").and_then(|v| v.as_str()) {
                Some(p) => p,
                None => {
                    self.send_response(seq, "launch", false, json!({"error": "no program specified"}));
                    return;
                }
            };
            self.source_file = path.to_string();
            match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(e) => {
                    self.send_response(seq, "launch", false, json!({"error": e.to_string()}));
                    return;
                }
            }
        };

        let tokens = tokenize(&source);
        let program = match parse(&tokens) {
            Ok(p) => p,
            Err(e) => {
                self.send_response(seq, "launch", false, json!({"error": format!("{e}")}));
                return;
            }
        };

        let instructions = compile(&program);
        self.source_map = build_source_map(&instructions);
        self.interpreter = Some(Interpreter::new(&program));
        self.finished = false;

        // Set up test mode.
        self.test_mode = true;
        self.test_input = args
            .get("input")
            .and_then(|v| v.as_str())
            .map(|s| s.as_bytes().to_vec())
            .unwrap_or_default();
        self.test_expected = args
            .get("expectedOutput")
            .and_then(|v| v.as_str())
            .map(|s| s.as_bytes().to_vec())
            .unwrap_or_default();
        self.test_output_counter = 0;
        self.test_output_so_far = Vec::new();
        self.test_mismatch = None;

        self.send_response(seq, "launch", true, json!({}));
    }

    fn handle_configuration_done(&mut self, seq: u64) {
        self.send_response(seq, "configurationDone", true, json!({}));
        if self.interpreter.is_some() && !self.finished {
            self.send_event("stopped", json!({"reason": "entry", "threadId": 1, "allThreadsStopped": true}));
        } else {
            self.send_event("terminated", json!({}));
        }
    }

    fn handle_set_breakpoints(&mut self, seq: u64, args: &Value) {
        let lines: Vec<usize> = args
            .get("breakpoints")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|bp| bp.get("line").and_then(|l| l.as_u64()))
                    .map(|l| l as usize)
                    .collect()
            })
            .unwrap_or_default();

        let results = self.breakpoints.set_breakpoints(&self.source_map, &lines);

        let bp_array: Vec<Value> = results
            .iter()
            .map(|r| {
                let mut obj = json!({"verified": r.verified, "line": r.line});
                if let Some(msg) = &r.message {
                    obj["message"] = json!(msg);
                }
                obj
            })
            .collect();

        self.send_response(seq, "setBreakpoints", true, json!({"breakpoints": bp_array}));
    }

    fn handle_threads(&mut self, seq: u64) {
        self.send_response(
            seq,
            "threads",
            true,
            json!({"threads": [{"id": 1, "name": "main"}]}),
        );
    }

    fn handle_stack_trace(&mut self, seq: u64) {
        let frame = if let Some(interp) = &self.interpreter {
            let pos = interp.current_source_pos();
            let (line, col) = pos.map(|p| (p.line, p.column)).unwrap_or((1, 1));
            json!({
                "id": 1,
                "name": "main",
                "source": {"path": self.source_file},
                "line": line,
                "column": col,
            })
        } else {
            json!({"id": 1, "name": "main", "line": 1, "column": 1})
        };
        self.send_response(
            seq,
            "stackTrace",
            true,
            json!({"stackFrames": [frame], "totalFrames": 1}),
        );
    }

    fn handle_scopes(&mut self, seq: u64) {
        let mismatch_ref = self.test_mismatch.as_ref();
        let scopes: Vec<Value> = get_scopes(mismatch_ref)
            .into_iter()
            .map(|s| {
                json!({
                    "name": s.name,
                    "variablesReference": s.variables_reference,
                    "expensive": s.expensive,
                })
            })
            .collect();
        self.send_response(seq, "scopes", true, json!({"scopes": scopes}));
    }

    fn handle_variables(&mut self, seq: u64, args: &Value) {
        let scope_ref = args.get("variablesReference").and_then(|v| v.as_u64()).unwrap_or(0);

        let vars = if let Some(interp) = &self.interpreter {
            match scope_ref {
                SCOPE_CURRENT_CELL | SCOPE_MEMORY => {
                    get_variables(scope_ref, interp, None)
                }
                SCOPE_TEST_MISMATCH => {
                    get_variables(scope_ref, interp, self.test_mismatch.as_ref())
                }
                _ => vec![],
            }
        } else {
            vec![]
        };

        let var_array: Vec<Value> = vars
            .into_iter()
            .map(|v| {
                json!({
                    "name": v.name,
                    "value": v.value,
                    "variablesReference": v.variables_reference,
                })
            })
            .collect();

        self.send_response(seq, "variables", true, json!({"variables": var_array}));
    }

    fn handle_continue(&mut self, seq: u64) {
        self.send_response(seq, "continue", true, json!({"allThreadsContinued": true}));
        self.run_until_stop(false);
    }

    fn handle_next(&mut self, seq: u64) {
        self.send_response(seq, "next", true, json!({}));

        if self.interpreter.is_none() || self.finished {
            self.send_event("terminated", json!({}));
            return;
        }

        if self.test_mode {
            let stop = self.test_step();
            if stop {
                return;
            }
            // Check if finished after step.
            if self.finished {
                self.check_test_completion_after_finish();
                return;
            }
        } else {
            let mut output_buf = Vec::new();
            let result = {
                let interp = self.interpreter.as_mut().unwrap();
                interp.step(&mut std::io::empty(), &mut output_buf)
            };
            self.flush_output(&output_buf);
            match result {
                StepResult::Finished | StepResult::RuntimeError(_) => {
                    self.finished = true;
                    self.send_event("terminated", json!({}));
                    return;
                }
                _ => {}
            }
        }

        self.send_event(
            "stopped",
            json!({"reason": "step", "threadId": 1, "allThreadsStopped": true}),
        );
    }

    fn handle_step_out(&mut self, seq: u64) {
        self.send_response(seq, "stepOut", true, json!({}));
        self.run_until_stop(true);
    }

    /// Execute one step in testDebug mode.
    /// Returns `true` if we fired a stop event (mismatch/too-long), `false` to continue.
    fn test_step(&mut self) -> bool {
        let interp = self.interpreter.as_mut().unwrap();
        let mut input_cursor = std::io::Cursor::new(self.test_input.clone());
        let mut out_byte = Vec::new();
        let result = interp.step(&mut input_cursor, &mut out_byte);

        match result {
            StepResult::Output(byte) => {
                let n = self.test_output_counter;
                if n >= self.test_expected.len() {
                    // Extra output beyond expected.
                    self.test_mismatch = Some(MismatchInfo {
                        diverged_at_byte: n,
                        expected_byte: None,
                        actual_byte: Some(byte),
                        output_so_far: self.test_output_so_far.clone(),
                        expected_output: self.test_expected.clone(),
                    });
                    self.send_event(
                        "stopped",
                        json!({"reason": "outputTooLong", "threadId": 1, "allThreadsStopped": true}),
                    );
                    return true;
                }
                if byte != self.test_expected[n] {
                    // Mismatch.
                    self.test_mismatch = Some(MismatchInfo {
                        diverged_at_byte: n,
                        expected_byte: Some(self.test_expected[n]),
                        actual_byte: Some(byte),
                        output_so_far: self.test_output_so_far.clone(),
                        expected_output: self.test_expected.clone(),
                    });
                    self.send_event(
                        "stopped",
                        json!({"reason": "testMismatch", "threadId": 1, "allThreadsStopped": true}),
                    );
                    return true;
                }
                // Byte matched.
                self.test_output_so_far.push(byte);
                self.test_output_counter += 1;
                false
            }
            StepResult::Finished | StepResult::RuntimeError(_) => {
                self.finished = true;
                false
            }
            _ => false,
        }
    }

    /// After program finishes in testDebug, check if output was too short.
    fn check_test_completion_after_finish(&mut self) {
        if self.test_output_counter < self.test_expected.len() {
            self.test_mismatch = Some(MismatchInfo {
                diverged_at_byte: self.test_output_counter,
                expected_byte: Some(self.test_expected[self.test_output_counter]),
                actual_byte: None,
                output_so_far: self.test_output_so_far.clone(),
                expected_output: self.test_expected.clone(),
            });
            self.send_event(
                "stopped",
                json!({"reason": "outputTooShort", "threadId": 1, "allThreadsStopped": true}),
            );
        } else {
            self.send_event("terminated", json!({}));
        }
    }

    /// Run interpreter until a breakpoint, test mismatch, or program finish.
    /// If `exit_loop` is true, stop when we exit the current loop nesting level.
    fn run_until_stop(&mut self, exit_loop: bool) {
        if self.interpreter.is_none() || self.finished {
            self.send_event("terminated", json!({}));
            return;
        }

        let initial_depth = if exit_loop {
            loop_depth_at(
                self.interpreter.as_ref().unwrap().get_instructions(),
                self.interpreter.as_ref().unwrap().get_pc(),
            )
        } else {
            0
        };

        loop {
            let interp = self.interpreter.as_ref().unwrap();
            if interp.is_finished() {
                self.finished = true;
                break;
            }
            let current_pc = interp.get_pc();

            if self.breakpoints.is_at_breakpoint(current_pc) {
                self.send_event(
                    "stopped",
                    json!({"reason": "breakpoint", "threadId": 1, "allThreadsStopped": true}),
                );
                return;
            }

            if self.test_mode {
                if self.test_step() {
                    // Mismatch or too-long: stop event already sent.
                    return;
                }
                if self.finished {
                    break;
                }
            } else {
                let mut output_buf = Vec::new();
                let result = {
                    let interp = self.interpreter.as_mut().unwrap();
                    interp.step(&mut std::io::empty(), &mut output_buf)
                };
                self.flush_output(&output_buf);
                match result {
                    StepResult::Finished | StepResult::RuntimeError(_) => {
                        self.finished = true;
                        break;
                    }
                    _ => {}
                }
            }

            if exit_loop {
                let interp = self.interpreter.as_ref().unwrap();
                let depth = loop_depth_at(interp.get_instructions(), interp.get_pc());
                if depth < initial_depth {
                    self.send_event(
                        "stopped",
                        json!({"reason": "step", "threadId": 1, "allThreadsStopped": true}),
                    );
                    return;
                }
            }
        }

        if self.test_mode {
            self.check_test_completion_after_finish();
        } else {
            self.send_event("terminated", json!({}));
        }
    }

    fn flush_output(&mut self, buf: &[u8]) {
        if !buf.is_empty() {
            let text = String::from_utf8_lossy(buf);
            self.send_event("output", json!({"category": "stdout", "output": text}));
        }
    }

    fn send_response(&mut self, request_seq: u64, command: &str, success: bool, body: Value) {
        let seq = self.next_seq();
        self.transport.send_message(&json!({
            "seq": seq,
            "type": "response",
            "request_seq": request_seq,
            "success": success,
            "command": command,
            "body": body,
        }));
    }

    fn send_event(&mut self, event: &str, body: Value) {
        let seq = self.next_seq();
        self.transport.send_message(&json!({
            "seq": seq,
            "type": "event",
            "event": event,
            "body": body,
        }));
    }

    fn next_seq(&mut self) -> u64 {
        let s = self.seq;
        self.seq += 1;
        s
    }
}

fn loop_depth_at(instructions: &[bf_interpreter::Instruction], pc: usize) -> usize {
    let mut depth = 0usize;
    for instr in instructions.iter().take(pc) {
        match instr.opcode {
            Opcode::JumpIfZero(_) => depth += 1,
            Opcode::JumpIfNotZero(_) => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::DapTransport;
    use std::io::Cursor;

    fn encode_dap(msg: &Value) -> Vec<u8> {
        let body = msg.to_string();
        format!("Content-Length: {}\r\n\r\n{}", body.len(), body).into_bytes()
    }

    fn decode_all(data: &[u8]) -> Vec<Value> {
        let mut msgs = Vec::new();
        let mut i = 0;
        while i < data.len() {
            let cl_prefix = b"Content-Length: ";
            if let Some(pos) = data[i..].windows(cl_prefix.len()).position(|w| w == cl_prefix) {
                let start = i + pos + cl_prefix.len();
                let end = data[start..].iter().position(|&b| b == b'\r').unwrap_or(data.len() - start) + start;
                let length: usize = std::str::from_utf8(&data[start..end]).unwrap().trim().parse().unwrap();
                let body_start = end + 4;
                let body = &data[body_start..body_start + length];
                if let Ok(v) = serde_json::from_slice::<Value>(body) {
                    msgs.push(v);
                }
                i = body_start + length;
            } else {
                break;
            }
        }
        msgs
    }

    fn run_adapter_with(input_msgs: Vec<Value>) -> Vec<Value> {
        let mut input_buf = Vec::new();
        for msg in &input_msgs {
            input_buf.extend_from_slice(&encode_dap(msg));
        }

        let mut out = Vec::new();
        let out_ptr = &mut out as *mut Vec<u8>;
        struct RawWriter(*mut Vec<u8>);
        impl std::io::Write for RawWriter {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                unsafe { (*self.0).extend_from_slice(buf) };
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
        }
        let transport = DapTransport::from_reader_writer(Cursor::new(input_buf), RawWriter(out_ptr));

        let mut adapter = DapAdapter::new(transport);
        adapter.run();

        decode_all(&out)
    }

    #[test]
    fn initialize_returns_capabilities() {
        let msgs = run_adapter_with(vec![json!({
            "seq": 1, "type": "request", "command": "initialize",
            "arguments": {"clientName": "test"}
        })]);
        let response = msgs.iter().find(|m| m["type"] == "response" && m["command"] == "initialize");
        assert!(response.is_some());
        assert_eq!(response.unwrap()["success"], true);
        assert_eq!(response.unwrap()["body"]["supportsConfigurationDoneRequest"], true);
        let event = msgs.iter().find(|m| m["type"] == "event" && m["event"] == "initialized");
        assert!(event.is_some());
    }

    #[test]
    fn launch_and_continue_sends_terminated() {
        let msgs = run_adapter_with(vec![
            json!({"seq": 1, "type": "request", "command": "initialize", "arguments": {}}),
            json!({"seq": 2, "type": "request", "command": "launch", "arguments": {"programText": "+++"}}),
            json!({"seq": 3, "type": "request", "command": "configurationDone", "arguments": {}}),
            json!({"seq": 4, "type": "request", "command": "continue", "arguments": {"threadId": 1}}),
        ]);
        let terminated = msgs.iter().find(|m| m["type"] == "event" && m["event"] == "terminated");
        assert!(terminated.is_some(), "no terminated event; messages: {:?}", msgs);
    }

    #[test]
    fn test_debug_mismatch_fires_stopped_event() {
        // Program outputs 'A' (65 x '+' then '.'), but expected is 'B' → testMismatch.
        let program_text = "+".repeat(65) + ".";
        let msgs = run_adapter_with(vec![
            json!({"seq": 1, "type": "request", "command": "initialize", "arguments": {}}),
            json!({"seq": 2, "type": "request", "command": "launch", "arguments": {
                "request": "testDebug",
                "programText": program_text,
                "input": "",
                "expectedOutput": "B"
            }}),
            json!({"seq": 3, "type": "request", "command": "configurationDone", "arguments": {}}),
            json!({"seq": 4, "type": "request", "command": "continue", "arguments": {"threadId": 1}}),
        ]);
        let stopped = msgs.iter().find(|m| {
            m["type"] == "event" && m["event"] == "stopped"
                && m["body"]["reason"] == "testMismatch"
        });
        assert!(stopped.is_some(), "expected testMismatch stop: {:?}", msgs);
    }

    #[test]
    fn test_debug_output_too_short() {
        // Program outputs nothing (+++) but expected "XY" → outputTooShort.
        let msgs = run_adapter_with(vec![
            json!({"seq": 1, "type": "request", "command": "initialize", "arguments": {}}),
            json!({"seq": 2, "type": "request", "command": "launch", "arguments": {
                "request": "testDebug",
                "programText": "+++",
                "input": "",
                "expectedOutput": "XY"
            }}),
            json!({"seq": 3, "type": "request", "command": "configurationDone", "arguments": {}}),
            json!({"seq": 4, "type": "request", "command": "continue", "arguments": {"threadId": 1}}),
        ]);
        let stopped = msgs.iter().find(|m| {
            m["type"] == "event" && m["event"] == "stopped"
                && m["body"]["reason"] == "outputTooShort"
        });
        assert!(stopped.is_some(), "expected outputTooShort stop: {:?}", msgs);
    }
}
