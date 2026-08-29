//! PLC-113 — `plc ld --serve`: a synchronous, line-delimited JSON protocol
//! over stdio wrapping the deterministic runtime. The CLIENT drives pacing
//! (`tick` = one scan), so the server is fully deterministic — the tests
//! script ops, run the loop once over in-memory buffers, and assert the
//! event stream in order.
//!
//! Events: `ready`, `loaded`, `error`, `state`, `powerFlow`, `diagnostics`.

use std::io::Write;

use plc_cli::run_ld_serve;
use serde_json::{Value, json};

/// One rung: Start ──TON(PT=T#200ms)──(Q→Done).
const TIMER_LD: &str = r#"{
    "name": "TimerTest",
    "rungs": [
        {
            "branches": [{ "elements": [{ "name": "Start", "negated": false }] }],
            "outputs": [
                {
                    "kind": "block",
                    "fb_type": "TON",
                    "instance": "Delay",
                    "inputs": [
                        { "name": "IN", "value": "Start" },
                        { "name": "PT", "value": "T#200ms" }
                    ],
                    "outputs": [{ "name": "Q", "value": "Done" }]
                }
            ]
        }
    ]
}"#;

/// A scripted session: ops accumulate, run once, events assert in order.
struct Session {
    ops: Vec<u8>,
    events: Vec<Value>,
    cursor: usize,
}

impl Session {
    fn new() -> Self {
        Self {
            ops: Vec::new(),
            events: Vec::new(),
            cursor: 0,
        }
    }

    fn send(&mut self, op: Value) {
        writeln!(self.ops, "{op}").unwrap();
    }

    /// Run the whole script against a real serve loop.
    fn run(&mut self) {
        let mut output: Vec<u8> = Vec::new();
        let input = std::io::Cursor::new(std::mem::take(&mut self.ops));
        run_ld_serve(input, &mut output).expect("serve loop");
        self.events = String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).expect("event JSON"))
            .collect();
    }

    fn next_event(&mut self) -> Value {
        let event = self
            .events
            .get(self.cursor)
            .unwrap_or_else(|| panic!("no event left (cursor {})", self.cursor))
            .clone();
        self.cursor += 1;
        event
    }

    fn hello(&mut self) {
        self.send(json!({"op": "hello"}));
    }

    fn load(&mut self, json: &str) {
        self.send(json!({"op": "load", "json": json}));
    }

    fn set_interval(&mut self, ms: i64) {
        self.send(json!({"op": "setInterval", "ms": ms}));
    }

    fn set_input(&mut self, name: &str, value: bool) {
        self.send(json!({"op": "setInput", "name": name, "value": value}));
    }

    fn force(&mut self, name: &str, value: bool) {
        self.send(json!({"op": "force", "name": name, "value": value}));
    }

    fn tick(&mut self) {
        self.send(json!({"op": "tick"}));
    }

    /// Assert-and-consume helpers.
    fn expect_ready(&mut self) {
        let ready = self.next_event();
        assert_eq!(ready["event"], "ready", "{ready}");
        assert_eq!(ready["protocolVersion"], 1);
        let catalog = ready["fbCatalog"].as_array().expect("fbCatalog array");
        assert!(
            catalog.iter().any(|entry| entry["fbType"] == "TON"),
            "{catalog:?}"
        );
    }

    fn expect_loaded(&mut self) {
        let loaded = self.next_event();
        assert_eq!(loaded["event"], "loaded", "{loaded}");
        assert_eq!(loaded["ok"], true, "{loaded}");
    }

    fn expect_state_and_flow(&mut self) -> (Value, Value) {
        let state = self.next_event();
        assert_eq!(state["event"], "state", "{state}");
        let flow = self.next_event();
        assert_eq!(flow["event"], "powerFlow", "{flow}");
        (state, flow)
    }

    fn expect_exhausted(&mut self) {
        assert_eq!(
            self.cursor,
            self.events.len(),
            "unconsumed events: {:?}",
            &self.events[self.cursor..]
        );
    }
}

#[test]
fn hello_handshake_returns_ready_with_catalog() {
    let mut session = Session::new();
    session.hello();
    session.run();
    session.expect_ready();
    session.expect_exhausted();
}

#[test]
fn full_script_handshake_load_input_ticks() {
    let mut session = Session::new();
    session.hello();
    session.load(TIMER_LD);
    session.set_interval(100);
    session.set_input("Start", true);
    session.tick();
    session.tick();
    session.tick();
    session.run();

    session.expect_ready();
    session.expect_loaded();
    // Scan n runs at t = n·100ms; TON fires at et >= 200ms → F, F, T.
    let (state1, flow1) = session.expect_state_and_flow();
    assert_eq!(state1["scan"], 1);
    assert_eq!(state1["timeMs"], 100);
    assert!(
        state1["watch"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l == "Start = TRUE"),
        "staged input applies on scan: {:?}",
        state1["watch"]
    );
    let contacts = flow1["rungs"][0]["contact_energized"].as_array().unwrap();
    assert!(contacts[0][0] == true, "Start contact energized: {flow1}");

    let (state2, _) = session.expect_state_and_flow();
    assert_eq!(state2["timeMs"], 200);
    assert!(
        !state2["watch"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l == "Done = TRUE"),
        "t=200: timer not done yet: {:?}",
        state2["watch"]
    );

    let (state3, _) = session.expect_state_and_flow();
    assert!(
        state3["watch"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l == "Done = TRUE"),
        "Done TRUE by the third scan: {:?}",
        state3["watch"]
    );
    session.expect_exhausted();
}

#[test]
fn load_invalid_json_emits_error_event() {
    let mut session = Session::new();
    session.hello();
    session.load("{not json");
    session.run();
    session.expect_ready();
    let error = session.next_event();
    assert_eq!(error["event"], "error", "{error}");
    assert!(
        error["message"].as_str().unwrap().contains("invalid"),
        "{error}"
    );
    session.expect_exhausted();
}

#[test]
fn force_wins_over_program_writes_and_lists_forced() {
    let mut session = Session::new();
    session.hello();
    session.load(TIMER_LD);
    session.set_interval(100);
    session.force("Start", true);
    session.tick();
    session.run();

    session.expect_ready();
    session.expect_loaded();
    let (state, _) = session.expect_state_and_flow();
    assert!(
        state["watch"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l == "Start = TRUE"),
        "forced input applies: {:?}",
        state["watch"]
    );
    assert!(
        // The runtime table stores names case-insensitively (lowercased).
        state["forced"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f.as_str().unwrap().eq_ignore_ascii_case("Start")),
        "forced list names Start: {}",
        state["forced"]
    );
    session.expect_exhausted();
}

#[test]
fn reload_replaces_program_and_resets_clock() {
    let mut session = Session::new();
    session.hello();
    session.load(TIMER_LD);
    session.set_interval(100);
    session.tick();
    session.load(TIMER_LD);
    session.tick();
    session.run();

    session.expect_ready();
    session.expect_loaded();
    session.expect_state_and_flow();
    session.expect_loaded();
    let (state, _) = session.expect_state_and_flow();
    assert_eq!(state["scan"], 1, "reload resets the scan counter");
    assert_eq!(state["timeMs"], 100, "reload resets the virtual clock");
    session.expect_exhausted();
}

#[test]
fn load_with_structural_errors_emits_diagnostics() {
    let mut session = Session::new();
    session.hello();
    // Rung without outputs → LD0004 warning rides along after `loaded`.
    session.load(
        r#"{"name":"P","rungs":[{"branches":[{"elements":[{"name":"A","negated":false}]}],"outputs":[]}]}"#,
    );
    session.run();

    session.expect_ready();
    session.expect_loaded();
    let diagnostics = session.next_event();
    assert_eq!(diagnostics["event"], "diagnostics", "{diagnostics}");
    let items = diagnostics["items"].as_array().unwrap();
    assert!(items.iter().any(|d| d["code"] == "LD0004"), "{items:?}");
    session.expect_exhausted();
}

#[test]
fn tick_without_load_emits_error() {
    let mut session = Session::new();
    session.tick();
    session.run();
    let error = session.next_event();
    assert_eq!(error["event"], "error", "{error}");
    assert!(
        error["message"].as_str().unwrap().contains("no program"),
        "{error}"
    );
    session.expect_exhausted();
}

#[test]
fn unknown_op_emits_error() {
    let mut session = Session::new();
    session.hello();
    session.send(json!({"op": "definitelyNotAnOp"}));
    session.run();
    session.expect_ready();
    let error = session.next_event();
    assert_eq!(error["event"], "error", "{error}");
    session.expect_exhausted();
}

#[test]
fn set_interval_survives_reload() {
    let mut session = Session::new();
    session.hello();
    session.load(TIMER_LD);
    session.set_interval(50);
    session.tick();
    session.load(TIMER_LD);
    session.tick();
    session.run();

    session.expect_ready();
    session.expect_loaded();
    let (state1, _) = session.expect_state_and_flow();
    assert_eq!(state1["timeMs"], 50, "interval 50 applies");
    session.expect_loaded();
    let (state2, _) = session.expect_state_and_flow();
    assert_eq!(state2["timeMs"], 50, "interval kept across reload");
    session.expect_exhausted();
}

#[test]
fn error_diagnostics_reach_client_when_conversion_fails() {
    let mut session = Session::new();
    session.hello();
    // Unknown FB type → LD0003 error → conversion fails, but the codes
    // must still reach the client.
    session.load(
        r#"{"name":"P","rungs":[{"branches":[{"elements":[{"name":"A","negated":false}]}],
            "outputs":[{"kind":"block","fb_type":"NOPE","instance":"T1","inputs":[],"outputs":[]}]}]}"#,
    );
    session.run();

    session.expect_ready();
    let diagnostics = session.next_event();
    assert_eq!(diagnostics["event"], "diagnostics", "{diagnostics}");
    assert!(
        diagnostics["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "LD0003"),
        "{diagnostics}"
    );
    let error = session.next_event();
    assert_eq!(error["event"], "error", "{error}");
    session.expect_exhausted();
}

#[test]
fn unforce_removes_the_force() {
    let mut session = Session::new();
    session.hello();
    session.load(TIMER_LD);
    session.set_interval(100);
    session.force("Start", true);
    session.tick();
    session.send(json!({"op": "unforce", "name": "Start"}));
    session.tick();
    session.run();

    session.expect_ready();
    session.expect_loaded();
    let (state1, _) = session.expect_state_and_flow();
    assert!(
        state1["forced"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f.as_str() == Some("start")),
        "force listed: {}",
        state1["forced"]
    );
    let (state2, _) = session.expect_state_and_flow();
    assert_eq!(
        state2["forced"].as_array().unwrap().len(),
        0,
        "unforce clears the list: {}",
        state2["forced"]
    );
    session.expect_exhausted();
}

#[test]
fn ready_catalog_lists_every_standard_fb() {
    let mut session = Session::new();
    session.hello();
    session.run();
    let ready = session.next_event();
    let catalog = ready["fbCatalog"].as_array().unwrap();
    assert_eq!(catalog.len(), 8, "all standard FBs present: {catalog:?}");
    session.expect_exhausted();
}

#[test]
fn mutating_op_without_load_emits_error() {
    let mut session = Session::new();
    session.hello();
    session.set_input("Start", true);
    session.run();
    session.expect_ready();
    let error = session.next_event();
    assert_eq!(error["event"], "error", "{error}");
    assert!(
        error["message"].as_str().unwrap().contains("no program"),
        "{error}"
    );
    session.expect_exhausted();
}

/// Regression (PLC-113 review): `plc ld <file> --watch` must emit the
/// power-flow JSON — an `args.any()` scan used to eat the --watch flag.
#[test]
fn ld_watch_flag_dispatches_to_power_flow() {
    let binary = env!("CARGO_BIN_EXE_plc");
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/ld/motor_control.ld"
    );
    let output = std::process::Command::new(binary)
        .args(["ld", fixture, "--watch"])
        .output()
        .expect("spawn plc");
    assert!(output.status.success(), "exit: {}", output.status);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("\"rungs\""),
        "--watch must print power-flow JSON, got: {stdout}"
    );
}
