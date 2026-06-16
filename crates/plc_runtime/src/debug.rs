//! Threaded stepping-debug session over a [`Runtime`].
//!
//! A whole logic scan normally runs atomically, so to implement breakpoints and
//! single-stepping the program runs on a dedicated worker thread that *blocks*
//! at each pause point. The worker and its driver exchange messages over two
//! channels: the worker emits [`PauseEvent`]s (where it stopped + a live
//! variable snapshot) and waits for a [`DebugCommand`] (continue / step / quit).
//!
//! The worker owns the `Runtime` outright — the driver never touches it — so
//! there is no shared mutable state to synchronize. The execution-control logic
//! lives in [`ChannelHook`], which the interpreter calls before every statement
//! via the `DebugHook` trait.

use std::collections::{BTreeSet, HashMap};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use crate::interp::{DebugHook, FbInstance, StepAction};
use crate::{Runtime, Value, VariableTable, display_watch};

/// Why execution paused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseReason {
    /// Stopped because the statement's line has a breakpoint.
    Breakpoint,
    /// Stopped because a step (over / into / out) reached its target.
    Step,
}

/// A point at which execution paused, with a snapshot of live state.
#[derive(Debug, Clone)]
pub struct PauseEvent {
    /// 1-based source line of the statement about to run.
    pub line: u32,
    /// 0-based scan cycle in which the pause occurred.
    pub scan: u64,
    /// Why the pause happened.
    pub reason: PauseReason,
    /// Declared scalar variables in source order, as `(name, rendered value)`.
    pub variables: Vec<(String, String)>,
    /// Declared function-block instances, each with its output members rendered.
    pub fbs: Vec<(String, Vec<(String, String)>)>,
}

/// A command sent to a paused session to resume it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugCommand {
    /// Run until the next breakpoint (or completion).
    Continue,
    /// Run to the next statement at the same nesting level or shallower.
    StepOver,
    /// Run to the very next statement (descending into nested blocks).
    StepIn,
    /// Run until execution returns to a shallower nesting level.
    StepOut,
    /// Stop the session and end execution.
    Disconnect,
}

/// A live stepping-debug session driving a `Runtime` on a worker thread.
pub struct DebugSession {
    events: Option<Receiver<PauseEvent>>,
    commands: Sender<DebugCommand>,
    worker: Option<JoinHandle<()>>,
}

impl DebugSession {
    /// Start executing `source` for up to `scans` cycles with the given
    /// breakpoint lines (1-based) armed. Execution begins immediately on a
    /// worker thread and pauses at the first breakpoint, emitting a
    /// [`PauseEvent`]; with no breakpoints it runs to completion.
    pub fn launch(source: &str, scans: u64, breakpoints: BTreeSet<u32>) -> Self {
        let (event_tx, event_rx) = mpsc::channel();
        let (command_tx, command_rx) = mpsc::channel();
        let source = source.to_owned();

        let worker = thread::Builder::new()
            .name("plc-debug".to_owned())
            .spawn(move || {
                let mut runtime = Runtime::from_source(&source);
                let mut hook = ChannelHook {
                    breakpoints,
                    mode: StepMode::Run,
                    scan: 0,
                    declared: runtime.declared_order().to_vec(),
                    declared_fbs: runtime.declared_fbs().to_vec(),
                    events: event_tx,
                    commands: command_rx,
                    stopped: false,
                };
                runtime.run_scans_with_hook(&mut hook, scans);
                // Dropping `hook` here closes the event channel, which the
                // driver observes as session termination.
            })
            .expect("spawn debug worker");

        Self {
            events: Some(event_rx),
            commands: command_tx,
            worker: Some(worker),
        }
    }

    /// Take ownership of the pause-event receiver (e.g. to move it into a
    /// forwarder thread). Returns `None` if already taken.
    pub fn take_events(&mut self) -> Option<Receiver<PauseEvent>> {
        self.events.take()
    }

    /// Borrow the pause-event receiver without taking it.
    pub fn events(&self) -> Option<&Receiver<PauseEvent>> {
        self.events.as_ref()
    }

    /// Send a resume command to the paused worker. A no-op if the worker has
    /// already finished.
    pub fn send(&self, command: DebugCommand) {
        let _ = self.commands.send(command);
    }
}

impl Drop for DebugSession {
    fn drop(&mut self) {
        // Unblock the worker if it is paused, then wait for it to wind down so
        // the thread does not outlive the session.
        let _ = self.commands.send(DebugCommand::Disconnect);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// How the session should decide where to pause next.
#[derive(Clone, Copy)]
enum StepMode {
    /// Pause only on breakpoints.
    Run,
    /// Pause at the very next statement.
    In,
    /// Pause at the next statement at depth `<= target`.
    Over(u32),
    /// Pause at the next statement at depth `< target`.
    Out(u32),
}

/// The `DebugHook` the interpreter calls before each statement. It decides
/// whether to pause, snapshots live state, blocks for the next command, and
/// updates the step mode accordingly.
struct ChannelHook {
    breakpoints: BTreeSet<u32>,
    mode: StepMode,
    scan: u64,
    declared: Vec<String>,
    declared_fbs: Vec<String>,
    events: Sender<PauseEvent>,
    commands: Receiver<DebugCommand>,
    stopped: bool,
}

impl ChannelHook {
    fn should_pause(&self, line: u32, depth: u32) -> Option<PauseReason> {
        if self.breakpoints.contains(&line) {
            return Some(PauseReason::Breakpoint);
        }
        let stepping = match self.mode {
            StepMode::Run => false,
            StepMode::In => true,
            StepMode::Over(target) => depth <= target,
            StepMode::Out(target) => depth < target,
        };
        stepping.then_some(PauseReason::Step)
    }

    fn snapshot_variables(&self, vars: &VariableTable) -> Vec<(String, String)> {
        self.declared
            .iter()
            .map(|name| {
                let value = vars.get(name).cloned().unwrap_or(Value::Unknown);
                (name.clone(), display_watch(&value))
            })
            .collect()
    }

    fn snapshot_fbs(
        &self,
        fbs: &HashMap<String, FbInstance>,
    ) -> Vec<(String, Vec<(String, String)>)> {
        self.declared_fbs
            .iter()
            .filter_map(|name| {
                let instance = fbs.get(&name.to_ascii_lowercase())?;
                let members = instance
                    .members()
                    .into_iter()
                    .map(|(member, value)| (member, display_watch(&value)))
                    .collect();
                Some((name.clone(), members))
            })
            .collect()
    }
}

impl DebugHook for ChannelHook {
    fn at_statement(
        &mut self,
        line: u32,
        depth: u32,
        vars: &VariableTable,
        fbs: &HashMap<String, FbInstance>,
    ) -> StepAction {
        if self.stopped {
            return StepAction::Stop;
        }
        let Some(reason) = self.should_pause(line, depth) else {
            return StepAction::Continue;
        };

        let event = PauseEvent {
            line,
            scan: self.scan,
            reason,
            variables: self.snapshot_variables(vars),
            fbs: self.snapshot_fbs(fbs),
        };
        if self.events.send(event).is_err() {
            // Driver hung up: stop cleanly.
            self.stopped = true;
            return StepAction::Stop;
        }

        // Block until the driver tells us how to resume.
        match self.commands.recv() {
            Ok(DebugCommand::Continue) => {
                self.mode = StepMode::Run;
                StepAction::Continue
            }
            Ok(DebugCommand::StepOver) => {
                self.mode = StepMode::Over(depth);
                StepAction::Continue
            }
            Ok(DebugCommand::StepIn) => {
                self.mode = StepMode::In;
                StepAction::Continue
            }
            Ok(DebugCommand::StepOut) => {
                self.mode = StepMode::Out(depth);
                StepAction::Continue
            }
            Ok(DebugCommand::Disconnect) | Err(_) => {
                self.stopped = true;
                StepAction::Stop
            }
        }
    }

    fn enter_scan(&mut self, scan: u64) {
        self.scan = scan;
    }

    fn is_stopped(&self) -> bool {
        self.stopped
    }
}
