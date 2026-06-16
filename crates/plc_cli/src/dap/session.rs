//! The DAP session state machine: turns editor requests into responses/events
//! and drives a [`DebugSession`] worker. Single-threaded — every method is
//! called from the one event loop in [`super::run`], so the writer is never
//! shared across threads.

use std::collections::BTreeSet;
use std::io::{self, Write};
use std::ops::ControlFlow;
use std::path::Path;
use std::sync::mpsc::Sender;
use std::thread;

use plc_cli::DEFAULT_SCANS;
use plc_runtime::{DebugCommand, DebugSession, PauseEvent, PauseReason};
use serde_json::{Value, json};

use super::Incoming;
use super::protocol::write_message;

/// The single (synthetic) thread the adapter reports to the editor.
const THREAD_ID: i64 = 1;
/// `variablesReference` for the scalar locals scope.
const LOCALS_REF: i64 = 1;
/// `variablesReference` for the function-block outputs scope.
const FBS_REF: i64 = 2;

#[derive(Clone)]
struct LaunchConfig {
    program: String,
    scans: u64,
    /// `true` for "Run Without Debugging" (Ctrl+F5): run straight through,
    /// ignoring breakpoints.
    no_debug: bool,
}

/// DAP session state. Execution starts only once *both* `launch` and
/// `configurationDone` have arrived, so breakpoints set during configuration
/// are armed before the worker runs.
pub struct Session {
    seq: i64,
    breakpoints: BTreeSet<u32>,
    launch_config: Option<LaunchConfig>,
    configuration_done: bool,
    debug: Option<DebugSession>,
    last_pause: Option<PauseEvent>,
    terminated: bool,
}

impl Session {
    pub fn new() -> Self {
        Self {
            seq: 0,
            breakpoints: BTreeSet::new(),
            launch_config: None,
            configuration_done: false,
            debug: None,
            last_pause: None,
            terminated: false,
        }
    }

    /// Handle one request from the editor. Returns `Break` when the session
    /// should end (disconnect/terminate).
    pub fn handle_client(
        &mut self,
        message: &Value,
        incoming_tx: &Sender<Incoming>,
        writer: &mut dyn Write,
    ) -> io::Result<ControlFlow<()>> {
        let command = message.get("command").and_then(Value::as_str).unwrap_or("");
        let request_seq = message.get("seq").and_then(Value::as_i64).unwrap_or(0);
        let arguments = message.get("arguments").cloned().unwrap_or(Value::Null);

        match command {
            "initialize" => {
                self.respond(
                    writer,
                    request_seq,
                    command,
                    json!({
                        "supportsConfigurationDoneRequest": true,
                        "supportsTerminateRequest": true,
                        "supportsEvaluateForHovers": true,
                    }),
                )?;
                self.event(writer, "initialized", Value::Null)?;
            }
            "setBreakpoints" => {
                let lines = parse_breakpoint_lines(&arguments);
                self.breakpoints = lines.iter().copied().collect();
                let verified: Vec<Value> = lines
                    .iter()
                    .map(|line| json!({ "verified": true, "line": line }))
                    .collect();
                self.respond(
                    writer,
                    request_seq,
                    command,
                    json!({ "breakpoints": verified }),
                )?;
            }
            "setExceptionBreakpoints" => {
                self.respond(writer, request_seq, command, json!({ "breakpoints": [] }))?;
            }
            "configurationDone" => {
                self.configuration_done = true;
                self.respond(writer, request_seq, command, Value::Null)?;
                self.maybe_start(incoming_tx, writer)?;
            }
            "launch" => {
                let program = arguments
                    .get("program")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                let scans = arguments
                    .get("scans")
                    .and_then(Value::as_u64)
                    .unwrap_or(DEFAULT_SCANS);
                let no_debug = arguments
                    .get("noDebug")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                self.launch_config = Some(LaunchConfig {
                    program,
                    scans,
                    no_debug,
                });
                self.respond(writer, request_seq, command, Value::Null)?;
                self.maybe_start(incoming_tx, writer)?;
            }
            "threads" => {
                self.respond(
                    writer,
                    request_seq,
                    command,
                    json!({ "threads": [ { "id": THREAD_ID, "name": "plc" } ] }),
                )?;
            }
            "stackTrace" => {
                let body = self.stack_trace_body();
                self.respond(writer, request_seq, command, body)?;
            }
            "scopes" => {
                self.respond(
                    writer,
                    request_seq,
                    command,
                    json!({
                        "scopes": [
                            { "name": "Locals", "variablesReference": LOCALS_REF, "expensive": false },
                            { "name": "Function Blocks", "variablesReference": FBS_REF, "expensive": false }
                        ]
                    }),
                )?;
            }
            "variables" => {
                let reference = arguments
                    .get("variablesReference")
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                let body = self.variables_body(reference);
                self.respond(writer, request_seq, command, body)?;
            }
            "continue" => {
                self.send_command(DebugCommand::Continue);
                self.respond(
                    writer,
                    request_seq,
                    command,
                    json!({ "allThreadsContinued": true }),
                )?;
            }
            "next" => {
                self.send_command(DebugCommand::StepOver);
                self.respond(writer, request_seq, command, Value::Null)?;
            }
            "stepIn" => {
                self.send_command(DebugCommand::StepIn);
                self.respond(writer, request_seq, command, Value::Null)?;
            }
            "stepOut" => {
                self.send_command(DebugCommand::StepOut);
                self.respond(writer, request_seq, command, Value::Null)?;
            }
            "evaluate" => match self.evaluate(&arguments) {
                Some(body) => self.respond(writer, request_seq, command, body)?,
                None => self.respond_error(writer, request_seq, command, "not available")?,
            },
            "disconnect" | "terminate" => {
                self.send_command(DebugCommand::Disconnect);
                self.respond(writer, request_seq, command, Value::Null)?;
                return Ok(ControlFlow::Break(()));
            }
            // Be lenient about optional/unknown requests so the session survives.
            _ => {
                self.respond(writer, request_seq, command, Value::Null)?;
            }
        }
        Ok(ControlFlow::Continue(()))
    }

    /// Emit a `stopped` event for a worker pause and cache its snapshot so the
    /// follow-up `stackTrace`/`scopes`/`variables` requests can answer.
    pub fn handle_pause(&mut self, event: PauseEvent, writer: &mut dyn Write) -> io::Result<()> {
        let reason = match event.reason {
            PauseReason::Breakpoint => "breakpoint",
            PauseReason::Step => "step",
        };
        self.event(
            writer,
            "output",
            json!({
                "category": "console",
                "output": format!("\u{23f8} scan {} \u{00b7} line {}\n", event.scan, event.line),
            }),
        )?;
        self.last_pause = Some(event);
        self.event(
            writer,
            "stopped",
            json!({ "reason": reason, "threadId": THREAD_ID, "allThreadsStopped": true }),
        )
    }

    /// Emit `terminated` + `exited` once the worker has finished all scans.
    pub fn handle_terminated(&mut self, writer: &mut dyn Write) -> io::Result<()> {
        if self.terminated {
            return Ok(());
        }
        self.terminated = true;
        self.event(
            writer,
            "output",
            json!({ "category": "console", "output": "PLC program finished.\n" }),
        )?;
        self.event(writer, "terminated", Value::Null)?;
        self.event(writer, "exited", json!({ "exitCode": 0 }))
    }

    /// Start the worker once both launch config and `configurationDone` are in.
    fn maybe_start(
        &mut self,
        incoming_tx: &Sender<Incoming>,
        writer: &mut dyn Write,
    ) -> io::Result<()> {
        if self.debug.is_some() || !self.configuration_done {
            return Ok(());
        }
        let Some(config) = self.launch_config.clone() else {
            return Ok(());
        };

        let source = match crate::read_source(Path::new(&config.program)) {
            Ok(text) => text,
            Err(error) => {
                self.event(
                    writer,
                    "output",
                    json!({ "category": "stderr", "output": format!("{error}\n") }),
                )?;
                self.event(writer, "terminated", Value::Null)?;
                self.event(writer, "exited", json!({ "exitCode": 1 }))?;
                return Ok(());
            }
        };

        self.event(
            writer,
            "output",
            json!({
                "category": "console",
                "output": format!("Debugging {} for {} scan cycle(s)\n", config.program, config.scans),
            }),
        )?;

        // "Run Without Debugging" (noDebug) ignores breakpoints: arm none so the
        // program runs straight through to completion.
        let breakpoints = if config.no_debug {
            BTreeSet::new()
        } else {
            self.breakpoints.clone()
        };
        let mut session = DebugSession::launch(&source, config.scans, breakpoints);
        if let Some(events) = session.take_events() {
            // Forward worker pauses/termination into the unified event loop.
            let pause_tx = incoming_tx.clone();
            thread::spawn(move || {
                while let Ok(event) = events.recv() {
                    if pause_tx.send(Incoming::Pause(event)).is_err() {
                        return;
                    }
                }
                let _ = pause_tx.send(Incoming::Terminated);
            });
        }
        self.debug = Some(session);
        Ok(())
    }

    fn stack_trace_body(&self) -> Value {
        let (line, scan) = self
            .last_pause
            .as_ref()
            .map(|pause| (pause.line, pause.scan))
            .unwrap_or((1, 0));
        let program = self
            .launch_config
            .as_ref()
            .map(|config| config.program.clone())
            .unwrap_or_default();
        json!({
            "stackFrames": [ {
                "id": 1,
                "name": format!("scan {scan} @ line {line}"),
                "line": line,
                "column": 1,
                "source": { "name": file_name(&program), "path": program }
            } ],
            "totalFrames": 1
        })
    }

    fn variables_body(&self, reference: i64) -> Value {
        let Some(pause) = self.last_pause.as_ref() else {
            return json!({ "variables": [] });
        };
        let variables: Vec<Value> = match reference {
            LOCALS_REF => pause
                .variables
                .iter()
                .map(|(name, value)| json!({ "name": name, "value": value, "variablesReference": 0 }))
                .collect(),
            FBS_REF => pause
                .fbs
                .iter()
                .flat_map(|(fb, members)| {
                    members.iter().map(move |(member, value)| {
                        json!({
                            "name": format!("{fb}.{member}"),
                            "value": value,
                            "variablesReference": 0
                        })
                    })
                })
                .collect(),
            _ => Vec::new(),
        };
        json!({ "variables": variables })
    }

    fn evaluate(&self, arguments: &Value) -> Option<Value> {
        let expression = arguments.get("expression").and_then(Value::as_str)?.trim();
        let pause = self.last_pause.as_ref()?;
        let value = pause
            .variables
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(expression))
            .map(|(_, value)| value.clone())
            .or_else(|| {
                let (instance, member) = expression.split_once('.')?;
                pause
                    .fbs
                    .iter()
                    .find(|(fb, _)| fb.eq_ignore_ascii_case(instance))
                    .and_then(|(_, members)| {
                        members
                            .iter()
                            .find(|(name, _)| name.eq_ignore_ascii_case(member))
                            .map(|(_, value)| value.clone())
                    })
            })?;
        Some(json!({ "result": value, "variablesReference": 0 }))
    }

    fn send_command(&self, command: DebugCommand) {
        if let Some(session) = &self.debug {
            session.send(command);
        }
    }

    fn next_seq(&mut self) -> i64 {
        self.seq += 1;
        self.seq
    }

    fn respond(
        &mut self,
        writer: &mut dyn Write,
        request_seq: i64,
        command: &str,
        body: Value,
    ) -> io::Result<()> {
        let seq = self.next_seq();
        let mut message = json!({
            "seq": seq,
            "type": "response",
            "request_seq": request_seq,
            "success": true,
            "command": command,
        });
        if !body.is_null() {
            message["body"] = body;
        }
        write_message(writer, &message)
    }

    fn respond_error(
        &mut self,
        writer: &mut dyn Write,
        request_seq: i64,
        command: &str,
        reason: &str,
    ) -> io::Result<()> {
        let seq = self.next_seq();
        let message = json!({
            "seq": seq,
            "type": "response",
            "request_seq": request_seq,
            "success": false,
            "command": command,
            "message": reason,
        });
        write_message(writer, &message)
    }

    fn event(&mut self, writer: &mut dyn Write, event: &str, body: Value) -> io::Result<()> {
        let seq = self.next_seq();
        let mut message = json!({ "seq": seq, "type": "event", "event": event });
        if !body.is_null() {
            message["body"] = body;
        }
        write_message(writer, &message)
    }
}

/// Parse `setBreakpoints` arguments into 1-based source lines, accepting either
/// the `breakpoints: [{ line }]` or legacy `lines: [n]` shape.
fn parse_breakpoint_lines(arguments: &Value) -> Vec<u32> {
    if let Some(breakpoints) = arguments.get("breakpoints").and_then(Value::as_array) {
        return breakpoints
            .iter()
            .filter_map(|bp| bp.get("line").and_then(Value::as_u64))
            .map(|line| line as u32)
            .collect();
    }
    arguments
        .get("lines")
        .and_then(Value::as_array)
        .map(|lines| {
            lines
                .iter()
                .filter_map(Value::as_u64)
                .map(|line| line as u32)
                .collect()
        })
        .unwrap_or_default()
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_owned()
}
