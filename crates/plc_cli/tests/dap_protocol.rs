//! End-to-end Debug Adapter Protocol coverage: spawn the real `plc debug`
//! binary and drive a scripted DAP session over its stdio pipes, exercising the
//! framing, the configuration handshake, breakpoint pausing, live variables,
//! a single step, and clean termination.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};

use serde_json::{Value, json};

const PROG: &str = "\
PROGRAM P
VAR
  a : INT := 0;
  b : INT := 0;
  c : INT := 0;
END_VAR
a := 1;
b := 2;
c := 3;
END_PROGRAM
";

fn line_of(needle: &str) -> u64 {
    let index = PROG.find(needle).expect("needle present");
    (PROG[..index].matches('\n').count() + 1) as u64
}

fn send(stdin: &mut ChildStdin, seq: i64, command: &str, arguments: Value) {
    let message = json!({
        "seq": seq,
        "type": "request",
        "command": command,
        "arguments": arguments,
    });
    let body = serde_json::to_vec(&message).unwrap();
    write!(stdin, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
    stdin.write_all(&body).unwrap();
    stdin.flush().unwrap();
}

fn read_message<R: BufRead>(reader: &mut R) -> Value {
    let mut length = None;
    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line).unwrap();
        assert!(read > 0, "unexpected EOF from the debug adapter");
        let header = line.trim_end_matches(['\r', '\n']);
        if header.is_empty() {
            break;
        }
        if let Some(value) = header.strip_prefix("Content-Length:") {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let length = length.expect("Content-Length header");
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body).unwrap();
    serde_json::from_slice(&body).unwrap()
}

/// Read messages until the matching response arrives (skipping events).
fn wait_response<R: BufRead>(reader: &mut R, command: &str) -> Value {
    for _ in 0..64 {
        let message = read_message(reader);
        if message["type"] == "response" && message["command"] == command {
            assert_eq!(
                message["success"], true,
                "response to {command} should succeed: {message}"
            );
            return message;
        }
    }
    panic!("did not observe a response to {command}");
}

/// Read messages until the matching event arrives (skipping responses/output).
fn wait_event<R: BufRead>(reader: &mut R, event: &str) -> Value {
    for _ in 0..64 {
        let message = read_message(reader);
        if message["type"] == "event" && message["event"] == event {
            return message;
        }
    }
    panic!("did not observe a {event} event");
}

fn spawn_adapter() -> Child {
    Command::new(env!("CARGO_BIN_EXE_plc"))
        .arg("debug")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn plc debug")
}

#[test]
fn full_stepping_session_over_dap() {
    let program = tempfile::Builder::new().suffix(".st").tempfile().unwrap();
    std::fs::write(program.path(), PROG).unwrap();
    let program_path = program.path().to_string_lossy().to_string();

    let mut child = spawn_adapter();
    let mut stdin = child.stdin.take().unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());

    // initialize -> capabilities + initialized event.
    send(
        &mut stdin,
        1,
        "initialize",
        json!({ "adapterID": "plc-st" }),
    );
    let init = wait_response(&mut reader, "initialize");
    assert_eq!(init["body"]["supportsConfigurationDoneRequest"], true);
    wait_event(&mut reader, "initialized");

    // launch (deferred) -> setBreakpoints -> configurationDone (starts the run).
    send(
        &mut stdin,
        2,
        "launch",
        json!({ "program": program_path, "scans": 1 }),
    );
    wait_response(&mut reader, "launch");

    let breakpoint = line_of("b := 2;");
    send(
        &mut stdin,
        3,
        "setBreakpoints",
        json!({
            "source": { "path": program_path },
            "breakpoints": [ { "line": breakpoint } ],
        }),
    );
    let bp_response = wait_response(&mut reader, "setBreakpoints");
    assert_eq!(bp_response["body"]["breakpoints"][0]["verified"], true);

    send(&mut stdin, 4, "configurationDone", json!({}));
    wait_response(&mut reader, "configurationDone");

    // Worker runs and pauses at the breakpoint.
    let stopped = wait_event(&mut reader, "stopped");
    assert_eq!(stopped["body"]["reason"], "breakpoint");

    // Live variables at the breakpoint: a := 1 has run; b/c not yet.
    send(
        &mut stdin,
        5,
        "variables",
        json!({ "variablesReference": 1 }),
    );
    let variables = wait_response(&mut reader, "variables");
    let names: Vec<(String, String)> = variables["body"]["variables"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            (
                v["name"].as_str().unwrap().to_owned(),
                v["value"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert!(
        names.contains(&("a".to_owned(), "1".to_owned())),
        "{names:?}"
    );
    assert!(
        names.contains(&("b".to_owned(), "0".to_owned())),
        "{names:?}"
    );

    // Step over -> stop on the next statement, confirmed via stackTrace line.
    send(&mut stdin, 6, "next", json!({ "threadId": 1 }));
    wait_response(&mut reader, "next");
    let step_stop = wait_event(&mut reader, "stopped");
    assert_eq!(step_stop["body"]["reason"], "step");

    send(&mut stdin, 7, "stackTrace", json!({ "threadId": 1 }));
    let stack = wait_response(&mut reader, "stackTrace");
    assert_eq!(stack["body"]["stackFrames"][0]["line"], line_of("c := 3;"));

    // Continue -> run to completion -> terminated + exited.
    send(&mut stdin, 8, "continue", json!({ "threadId": 1 }));
    wait_response(&mut reader, "continue");
    wait_event(&mut reader, "terminated");
    let exited = wait_event(&mut reader, "exited");
    assert_eq!(exited["body"]["exitCode"], 0);

    send(&mut stdin, 9, "disconnect", json!({}));
    wait_response(&mut reader, "disconnect");

    drop(stdin);
    let _ = child.wait();
}

#[test]
fn run_without_debugging_ignores_breakpoints() {
    // "Run Without Debugging" (Ctrl+F5) sets noDebug:true; even with a breakpoint
    // set, the program must run straight through to termination without stopping.
    let program = tempfile::Builder::new().suffix(".st").tempfile().unwrap();
    std::fs::write(program.path(), PROG).unwrap();
    let program_path = program.path().to_string_lossy().to_string();

    let mut child = spawn_adapter();
    let mut stdin = child.stdin.take().unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());

    send(
        &mut stdin,
        1,
        "initialize",
        json!({ "adapterID": "plc-st" }),
    );
    wait_response(&mut reader, "initialize");
    wait_event(&mut reader, "initialized");

    send(
        &mut stdin,
        2,
        "launch",
        json!({ "program": program_path, "scans": 1, "noDebug": true }),
    );
    wait_response(&mut reader, "launch");

    // A breakpoint IS set, but noDebug must make it ineffective.
    send(
        &mut stdin,
        3,
        "setBreakpoints",
        json!({
            "source": { "path": program_path },
            "breakpoints": [ { "line": line_of("b := 2;") } ],
        }),
    );
    wait_response(&mut reader, "setBreakpoints");

    send(&mut stdin, 4, "configurationDone", json!({}));
    wait_response(&mut reader, "configurationDone");

    // No `stopped` event should arrive — the run goes straight to terminated.
    for _ in 0..64 {
        let message = read_message(&mut reader);
        assert_ne!(
            message["event"], "stopped",
            "noDebug run must not stop at a breakpoint: {message}"
        );
        if message["type"] == "event" && message["event"] == "terminated" {
            break;
        }
    }

    send(&mut stdin, 5, "disconnect", json!({}));
    wait_response(&mut reader, "disconnect");

    drop(stdin);
    let _ = child.wait();
}
