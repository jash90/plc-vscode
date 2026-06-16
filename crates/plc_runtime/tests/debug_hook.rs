//! Stepping-debug coverage for the threaded `DebugSession`: breakpoints, the
//! four step modes, live variable/FB snapshots, breakpoint re-arming across
//! scans, disconnect, and a guard that the no-hook path is unchanged.

use std::collections::BTreeSet;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use plc_runtime::{DebugCommand, DebugSession, PauseEvent, PauseReason, Runtime, Value};

/// A program exercising sequential assignments and a `FOR` loop. Breakpoints
/// are referenced by source text via [`line_of`] so the tests do not hard-code
/// line numbers.
const PROG: &str = "\
PROGRAM P
VAR
  a : INT := 0;
  b : INT := 0;
  i : INT := 0;
END_VAR
a := 1;
b := 2;
FOR i := 1 TO 3 DO
  a := a + i;
END_FOR;
b := a;
END_PROGRAM
";

/// A program that calls a `TON` so its outputs appear in the FB snapshot.
const FB_PROG: &str = "\
PROGRAM P
VAR
  go : BOOL := TRUE;
  done : BOOL := FALSE;
  fbT : TON;
END_VAR
fbT(IN := go, PT := T#0ms);
done := fbT.Q;
END_PROGRAM
";

/// 1-based source line of the first occurrence of `needle`.
fn line_of(source: &str, needle: &str) -> u32 {
    let index = source
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not found"));
    (source[..index].matches('\n').count() + 1) as u32
}

/// Receive the next pause event, failing the test rather than hanging forever.
fn recv(events: &Receiver<PauseEvent>) -> Option<PauseEvent> {
    events.recv_timeout(Duration::from_secs(5)).ok()
}

fn breakpoints(lines: &[u32]) -> BTreeSet<u32> {
    lines.iter().copied().collect()
}

fn var<'a>(event: &'a PauseEvent, name: &str) -> Option<&'a str> {
    event
        .variables
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, value)| value.as_str())
}

#[test]
fn pauses_at_breakpoint_line() {
    let bp = line_of(PROG, "b := 2;");
    let mut session = DebugSession::launch(PROG, 1, breakpoints(&[bp]));
    let events = session.take_events().unwrap();

    let event = recv(&events).expect("a pause");
    assert_eq!(event.line, bp);
    assert_eq!(event.reason, PauseReason::Breakpoint);

    session.send(DebugCommand::Continue);
    assert!(recv(&events).is_none(), "session should terminate");
}

#[test]
fn continue_runs_to_the_next_breakpoint() {
    let first = line_of(PROG, "a := 1;");
    let second = line_of(PROG, "b := a;");
    let mut session = DebugSession::launch(PROG, 1, breakpoints(&[first, second]));
    let events = session.take_events().unwrap();

    assert_eq!(recv(&events).unwrap().line, first);
    session.send(DebugCommand::Continue);
    assert_eq!(recv(&events).unwrap().line, second);
    session.send(DebugCommand::Continue);
    assert!(recv(&events).is_none());
}

#[test]
fn step_over_does_not_descend_into_for_body() {
    let bp = line_of(PROG, "FOR i");
    let mut session = DebugSession::launch(PROG, 1, breakpoints(&[bp]));
    let events = session.take_events().unwrap();

    assert_eq!(recv(&events).unwrap().line, bp);
    session.send(DebugCommand::StepOver);

    let next = recv(&events).expect("a pause after the loop");
    assert_eq!(next.line, line_of(PROG, "b := a;"));
    assert_ne!(next.line, line_of(PROG, "a := a + i;"));
}

#[test]
fn step_into_stops_on_first_inner_statement() {
    let bp = line_of(PROG, "FOR i");
    let mut session = DebugSession::launch(PROG, 1, breakpoints(&[bp]));
    let events = session.take_events().unwrap();

    assert_eq!(recv(&events).unwrap().line, bp);
    session.send(DebugCommand::StepIn);

    let inner = recv(&events).expect("a pause inside the loop");
    assert_eq!(inner.line, line_of(PROG, "a := a + i;"));
    assert_eq!(inner.reason, PauseReason::Step);
}

#[test]
fn step_out_returns_to_the_enclosing_level() {
    let bp = line_of(PROG, "FOR i");
    let mut session = DebugSession::launch(PROG, 1, breakpoints(&[bp]));
    let events = session.take_events().unwrap();

    assert_eq!(recv(&events).unwrap().line, bp);
    session.send(DebugCommand::StepIn);
    assert_eq!(recv(&events).unwrap().line, line_of(PROG, "a := a + i;"));

    session.send(DebugCommand::StepOut);
    let after = recv(&events).expect("a pause after the loop");
    assert_eq!(after.line, line_of(PROG, "b := a;"));
}

#[test]
fn snapshot_reflects_prior_mutations() {
    let bp = line_of(PROG, "b := a;");
    let mut session = DebugSession::launch(PROG, 1, breakpoints(&[bp]));
    let events = session.take_events().unwrap();

    let event = recv(&events).expect("a pause");
    // a = 1 + (1 + 2 + 3) = 7 by this point; b is still 2 (this stmt not yet run).
    assert_eq!(var(&event, "a"), Some("7"));
    assert_eq!(var(&event, "b"), Some("2"));
}

#[test]
fn snapshot_includes_function_block_outputs() {
    let bp = line_of(FB_PROG, "done := fbT.Q;");
    let mut session = DebugSession::launch(FB_PROG, 1, breakpoints(&[bp]));
    let events = session.take_events().unwrap();

    let event = recv(&events).expect("a pause");
    let fb = event
        .fbs
        .iter()
        .find(|(name, _)| name == "fbT")
        .expect("fbT in the FB snapshot");
    assert!(
        fb.1.iter().any(|(member, _)| member == "Q"),
        "fbT should expose a Q output: {:?}",
        fb.1
    );
}

#[test]
fn breakpoint_rearms_across_scans() {
    let bp = line_of(PROG, "a := 1;");
    let mut session = DebugSession::launch(PROG, 2, breakpoints(&[bp]));
    let events = session.take_events().unwrap();

    let first = recv(&events).expect("scan 0 pause");
    assert_eq!(first.line, bp);
    assert_eq!(first.scan, 0);

    session.send(DebugCommand::Continue);
    let second = recv(&events).expect("scan 1 pause");
    assert_eq!(second.line, bp);
    assert_eq!(second.scan, 1);

    session.send(DebugCommand::Continue);
    assert!(recv(&events).is_none());
}

#[test]
fn disconnect_stops_promptly() {
    let bp = line_of(PROG, "a := 1;");
    // 100 scans requested, but disconnect must end the session immediately.
    let mut session = DebugSession::launch(PROG, 100, breakpoints(&[bp]));
    let events = session.take_events().unwrap();

    assert!(recv(&events).is_some());
    session.send(DebugCommand::Disconnect);
    assert!(recv(&events).is_none(), "session should stop on disconnect");
}

#[test]
fn no_hook_execution_is_unchanged() {
    // Regression guard: the optional-hook plumbing must not alter normal runs.
    let mut runtime = Runtime::from_source(PROG);
    runtime.run_scans(1);
    assert_eq!(runtime.value("a"), Some(&Value::Int(7)));
    assert_eq!(runtime.value("b"), Some(&Value::Int(7)));
}
