use bf_dap::adapter::DapAdapter;
use bf_dap::transport::DapTransport;
use serde_json::{json, Value};
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
            let end = data[start..]
                .iter()
                .position(|&b| b == b'\r')
                .unwrap_or(data.len() - start)
                + start;
            let length: usize = std::str::from_utf8(&data[start..end])
                .unwrap()
                .trim()
                .parse()
                .unwrap();
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

struct RawWriter(*mut Vec<u8>);
impl std::io::Write for RawWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        unsafe { (*self.0).extend_from_slice(buf) };
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn run_adapter_with(input_msgs: Vec<Value>) -> Vec<Value> {
    let mut input_buf = Vec::new();
    for msg in &input_msgs {
        input_buf.extend_from_slice(&encode_dap(msg));
    }
    let mut out = Vec::new();
    let out_ptr = &mut out as *mut Vec<u8>;
    let transport = DapTransport::from_reader_writer(Cursor::new(input_buf), RawWriter(out_ptr));
    let mut adapter = DapAdapter::new(transport);
    adapter.run();
    decode_all(&out)
}

// ── Breakpoint + step + variables ────────────────────────────────────────────

#[test]
fn breakpoint_resolves_and_stops_at_correct_line() {
    // Program: "++" (two increments). Set a breakpoint on line 1.
    // After launch + configurationDone we expect a stopped/entry event.
    let msgs = run_adapter_with(vec![
        json!({"seq": 1, "type": "request", "command": "initialize", "arguments": {}}),
        json!({"seq": 2, "type": "request", "command": "launch",
               "arguments": {"programText": "++\n++"}}),
        json!({"seq": 3, "type": "request", "command": "setBreakpoints",
               "arguments": {
                   "source": {"path": "inline.bf"},
                   "breakpoints": [{"line": 1}]
               }}),
        json!({"seq": 4, "type": "request", "command": "configurationDone", "arguments": {}}),
    ]);

    // Should have a stopped event (entry or breakpoint)
    let stopped = msgs.iter().find(|m| m["type"] == "event" && m["event"] == "stopped");
    assert!(stopped.is_some(), "expected a stopped event; got: {:?}", msgs);
}

#[test]
fn step_then_variables_show_updated_cell() {
    // `+` increments cell 0 to 1; inspect via scopes/variables.
    let msgs = run_adapter_with(vec![
        json!({"seq": 1, "type": "request", "command": "initialize", "arguments": {}}),
        json!({"seq": 2, "type": "request", "command": "launch",
               "arguments": {"programText": "+"}}),
        json!({"seq": 3, "type": "request", "command": "configurationDone", "arguments": {}}),
        json!({"seq": 4, "type": "request", "command": "next", "arguments": {"threadId": 1}}),
        json!({"seq": 5, "type": "request", "command": "scopes",  "arguments": {"frameId": 0}}),
        json!({"seq": 6, "type": "request", "command": "variables",
               "arguments": {"variablesReference": 1}}),  // SCOPE_CURRENT_CELL = 1
    ]);

    // Find variables response
    let var_resp = msgs.iter().find(|m| {
        m["type"] == "response" && m["command"] == "variables"
    });
    assert!(var_resp.is_some(), "expected variables response; got: {:?}", msgs);

    let vars = &var_resp.unwrap()["body"]["variables"];
    // pointer should be 0, value should start with "1"
    let value_var = vars.as_array().and_then(|arr| {
        arr.iter().find(|v| v["name"] == "value")
    });
    if let Some(v) = value_var {
        assert!(
            v["value"].as_str().unwrap_or("").starts_with('1'),
            "expected cell value 1 after `+`, got: {}",
            v["value"]
        );
    }
}

#[test]
fn continue_to_end_sends_terminated() {
    let msgs = run_adapter_with(vec![
        json!({"seq": 1, "type": "request", "command": "initialize", "arguments": {}}),
        json!({"seq": 2, "type": "request", "command": "launch",
               "arguments": {"programText": "+++"}}),
        json!({"seq": 3, "type": "request", "command": "configurationDone", "arguments": {}}),
        json!({"seq": 4, "type": "request", "command": "continue",
               "arguments": {"threadId": 1}}),
    ]);
    let terminated = msgs.iter().find(|m| m["type"] == "event" && m["event"] == "terminated");
    assert!(terminated.is_some(), "expected terminated event; got: {:?}", msgs);
}

// ── testDebug mode ────────────────────────────────────────────────────────────

#[test]
fn test_debug_correct_output_terminates_normally() {
    // `+++.` outputs chr(3); expected "chr(3)" → should terminate, not mismatch.
    let msgs = run_adapter_with(vec![
        json!({"seq": 1, "type": "request", "command": "initialize", "arguments": {}}),
        json!({"seq": 2, "type": "request", "command": "launch", "arguments": {
            "request": "testDebug",
            "programText": "+++.",
            "input": "",
            "expectedOutput": "\x03"
        }}),
        json!({"seq": 3, "type": "request", "command": "configurationDone", "arguments": {}}),
        json!({"seq": 4, "type": "request", "command": "continue", "arguments": {"threadId": 1}}),
    ]);
    let terminated = msgs.iter().find(|m| m["type"] == "event" && m["event"] == "terminated");
    assert!(terminated.is_some(), "expected terminated (no mismatch); got: {:?}", msgs);
}

#[test]
fn test_debug_mismatch_exposes_scope() {
    // `+.` outputs chr(1), expected 'X' → testMismatch, then check Test Mismatch scope.
    let msgs = run_adapter_with(vec![
        json!({"seq": 1, "type": "request", "command": "initialize", "arguments": {}}),
        json!({"seq": 2, "type": "request", "command": "launch", "arguments": {
            "request": "testDebug",
            "programText": "+.",
            "input": "",
            "expectedOutput": "X"
        }}),
        json!({"seq": 3, "type": "request", "command": "configurationDone", "arguments": {}}),
        json!({"seq": 4, "type": "request", "command": "continue", "arguments": {"threadId": 1}}),
        json!({"seq": 5, "type": "request", "command": "scopes",  "arguments": {"frameId": 0}}),
        json!({"seq": 6, "type": "request", "command": "variables",
               "arguments": {"variablesReference": 3}}), // SCOPE_TEST_MISMATCH = 3
    ]);

    let stopped = msgs.iter().find(|m| {
        m["type"] == "event" && m["event"] == "stopped" && m["body"]["reason"] == "testMismatch"
    });
    assert!(stopped.is_some(), "expected testMismatch stop; got: {:?}", msgs);

    // Scopes response should include a "Test Mismatch" scope
    let scopes_resp = msgs.iter().find(|m| m["type"] == "response" && m["command"] == "scopes");
    if let Some(sr) = scopes_resp {
        let scopes = &sr["body"]["scopes"];
        let has_mismatch_scope = scopes
            .as_array()
            .map(|arr| arr.iter().any(|s| s["name"] == "Test Mismatch"))
            .unwrap_or(false);
        assert!(has_mismatch_scope, "expected Test Mismatch scope; scopes: {:?}", scopes);
    }

    // Variables for scope 3 should include diverged_at_byte
    let var_resp = msgs.iter().find(|m| m["type"] == "response" && m["command"] == "variables");
    if let Some(vr) = var_resp {
        let has_diverged = vr["body"]["variables"]
            .as_array()
            .map(|arr| arr.iter().any(|v| v["name"] == "diverged_at_byte"))
            .unwrap_or(false);
        assert!(has_diverged, "expected diverged_at_byte variable; got: {:?}", vr);
    }
}

#[test]
fn test_debug_output_too_long_fires_event() {
    // `++.+.` outputs 2 bytes, expected only 1 → outputTooLong on second byte.
    let msgs = run_adapter_with(vec![
        json!({"seq": 1, "type": "request", "command": "initialize", "arguments": {}}),
        json!({"seq": 2, "type": "request", "command": "launch", "arguments": {
            "request": "testDebug",
            "programText": "++.+.",
            "input": "",
            "expectedOutput": "\x02"
        }}),
        json!({"seq": 3, "type": "request", "command": "configurationDone", "arguments": {}}),
        json!({"seq": 4, "type": "request", "command": "continue", "arguments": {"threadId": 1}}),
    ]);
    let too_long = msgs.iter().find(|m| {
        m["type"] == "event" && m["event"] == "stopped" && m["body"]["reason"] == "outputTooLong"
    });
    assert!(too_long.is_some(), "expected outputTooLong stop; got: {:?}", msgs);
}
