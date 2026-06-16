//! Deterministic PLC scan-cycle runtime for PLC VS Code.
//!
//! The runtime models the classic PLC execution loop — input scan, logic scan,
//! output scan — over retained variable state. Programs are taken from
//! `plc_syntax` parse output and executed deterministically so tests and the
//! simulator UI observe identical behavior on every run.

use std::collections::HashMap;

mod bytecode;
mod clock;
pub mod counters;
mod debug;
pub mod edge;
mod engine;
mod interp;
pub mod stdlib;
pub mod timers;
mod value;

pub use bytecode::{BytecodeModule, Instruction, lower_module, lower_program};
pub use clock::VirtualClock;
pub use counters::{Ctd, Ctu, Ctud};
pub use debug::{DebugCommand, DebugSession, PauseEvent, PauseReason};
pub use edge::{FTrig, RTrig};
pub use engine::ScanRuntimeEngine;
pub use timers::{Tof, Ton, Tp};
pub use value::Value;

/// The phase of a single scan cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanPhase {
    Input,
    Logic,
    Output,
}

/// Retained variable state, addressed case-insensitively like IEC identifiers.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VariableTable {
    values: HashMap<String, Value>,
}

impl VariableTable {
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.values.get(&name.to_ascii_lowercase())
    }

    pub fn set(&mut self, name: &str, value: Value) {
        self.values.insert(name.to_ascii_lowercase(), value);
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Deterministically ordered (lowercased name, value) entries.
    pub fn entries(&self) -> Vec<(String, Value)> {
        let mut entries: Vec<(String, Value)> = self
            .values
            .iter()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();
        entries.sort_by(|left, right| left.0.cmp(&right.0));
        entries
    }
}

/// Inspection record for a single runtime variable.
#[derive(Debug, Clone, PartialEq)]
pub struct VariableSnapshot {
    pub name: String,
    pub value: Value,
    pub forced: bool,
}

/// Deterministic scan-cycle runtime over a single Structured Text program.
#[derive(Debug, Clone)]
pub struct Runtime {
    body: Vec<interp::Stmt>,
    state: VariableTable,
    pending_inputs: VariableTable,
    outputs: Vec<String>,
    /// Declared scalar variables in source order, for the watch snapshot.
    declared_order: Vec<String>,
    /// Declared function-block instance names in source order, for the debugger.
    declared_fbs: Vec<String>,
    /// Live standard function-block instances keyed by lowercased name.
    fbs: HashMap<String, interp::FbInstance>,
    scan_count: u64,
    clock: VirtualClock,
    forces: VariableTable,
}

impl Runtime {
    /// Build a runtime from Structured Text source, retaining declared outputs.
    pub fn from_source(text: &str) -> Self {
        let program = interp::build_program(text);
        let mut state = VariableTable::default();
        let mut outputs = Vec::new();
        let mut declared_order = Vec::new();
        let mut declared_fbs = Vec::new();
        let mut fbs = HashMap::new();

        for var in &program.vars {
            // Function-block instances are stateful objects, not scalar state.
            if var.is_fb {
                if let Some(instance) = interp::FbInstance::new(&var.type_name) {
                    fbs.insert(var.name.to_ascii_lowercase(), instance);
                    declared_fbs.push(var.name.clone());
                }
                continue;
            }
            // Cold-start initialization: the initializer if present, else the
            // type default.
            let value = var
                .init
                .clone()
                .unwrap_or_else(|| Value::type_default(&var.type_name));
            state.set(&var.name, value);
            declared_order.push(var.name.clone());
            if var.is_output {
                outputs.push(var.name.clone());
            }
        }

        Self {
            body: program.body,
            state,
            pending_inputs: VariableTable::default(),
            outputs,
            declared_order,
            declared_fbs,
            fbs,
            scan_count: 0,
            clock: VirtualClock::default(),
            forces: VariableTable::default(),
        }
    }

    /// Force a variable to a fixed value that overrides logic-scan writes until
    /// released. Forces are re-applied at the end of every logic scan.
    pub fn force(&mut self, name: &str, value: Value) {
        self.forces.set(name, value.clone());
        self.state.set(name, value);
    }

    /// Release a previously forced variable.
    pub fn unforce(&mut self, name: &str) {
        self.forces.values.remove(&name.to_ascii_lowercase());
    }

    /// Whether a variable is currently forced.
    pub fn is_forced(&self, name: &str) -> bool {
        self.forces.get(name).is_some()
    }

    /// Inspect the full retained state, including which variables are forced.
    pub fn inspect(&self) -> Vec<VariableSnapshot> {
        self.state
            .entries()
            .into_iter()
            .map(|(name, value)| VariableSnapshot {
                forced: self.forces.get(&name).is_some(),
                name,
                value,
            })
            .collect()
    }

    /// Current virtual time in milliseconds.
    pub fn now_ms(&self) -> i64 {
        self.clock.now_ms()
    }

    /// Read-only access to the virtual clock.
    pub fn clock(&self) -> &VirtualClock {
        &self.clock
    }

    /// Configure the per-scan virtual time increment.
    pub fn set_scan_interval_ms(&mut self, scan_interval_ms: i64) {
        self.clock.set_scan_interval_ms(scan_interval_ms);
    }

    /// Advance virtual time explicitly without running a scan.
    pub fn advance_time(&mut self, delta_ms: i64) {
        self.clock.advance(delta_ms);
    }

    /// Stage an input value to be latched at the next input scan.
    pub fn set_input(&mut self, name: &str, value: Value) {
        self.pending_inputs.set(name, value);
    }

    /// Read the current retained value of a variable.
    pub fn value(&self, name: &str) -> Option<&Value> {
        self.state.get(name)
    }

    /// Snapshot of the retained variable state.
    pub fn state(&self) -> &VariableTable {
        &self.state
    }

    /// Number of completed scan cycles.
    pub fn scan_count(&self) -> u64 {
        self.scan_count
    }

    /// Execute one full scan cycle: input scan, logic scan, output scan.
    ///
    /// Returns the output-scan snapshot (`name = value`) for the declared
    /// output variables.
    pub fn run_scan(&mut self) -> Vec<String> {
        self.clock.tick();
        self.scan_phase(ScanPhase::Input);
        self.scan_phase(ScanPhase::Logic);
        self.apply_forces();
        let snapshot = self.scan_phase_output();
        self.scan_count += 1;
        snapshot
    }

    /// Run `cycles` scan cycles, returning the final output snapshot.
    pub fn run_scans(&mut self, cycles: u64) -> Vec<String> {
        let mut snapshot = Vec::new();
        for _ in 0..cycles {
            snapshot = self.run_scan();
        }
        snapshot
    }

    /// Run up to `scans` scan cycles with a stepping-debug `hook` installed in
    /// each logic phase. Mirrors [`run_scan`](Self::run_scan) but routes the
    /// logic scan through the hook so it can pause/inspect/resume; stops early
    /// once the hook reports the session has been stopped.
    pub(crate) fn run_scans_with_hook(&mut self, hook: &mut dyn interp::DebugHook, scans: u64) {
        for _ in 0..scans {
            self.clock.tick();
            self.scan_phase(ScanPhase::Input);
            hook.enter_scan(self.scan_count);

            let body = std::mem::take(&mut self.body);
            let now_ms = self.clock.now_ms();
            let mut exec = interp::ExecState {
                vars: &mut self.state,
                fbs: &mut self.fbs,
                now_ms,
                hook: Some(&mut *hook),
                depth: 0,
            };
            interp::exec_block(&body, &mut exec);
            self.body = body;

            self.apply_forces();
            self.scan_count += 1;
            if hook.is_stopped() {
                break;
            }
        }
    }

    /// Declared scalar variable names in source order (for the debugger).
    pub(crate) fn declared_order(&self) -> &[String] {
        &self.declared_order
    }

    /// Declared function-block instance names in source order (for the debugger).
    pub(crate) fn declared_fbs(&self) -> &[String] {
        &self.declared_fbs
    }

    fn scan_phase(&mut self, phase: ScanPhase) {
        match phase {
            ScanPhase::Input => {
                let staged: Vec<(String, Value)> = self
                    .pending_inputs
                    .values
                    .iter()
                    .map(|(name, value)| (name.clone(), value.clone()))
                    .collect();
                for (name, value) in staged {
                    self.state.values.insert(name, value);
                }
            }
            ScanPhase::Logic => {
                // Move the body out so the interpreter can borrow `state`/`fbs`
                // mutably while walking the (disjoint) statement tree.
                let body = std::mem::take(&mut self.body);
                let now_ms = self.clock.now_ms();
                let mut exec = interp::ExecState {
                    vars: &mut self.state,
                    fbs: &mut self.fbs,
                    now_ms,
                    hook: None,
                    depth: 0,
                };
                interp::exec_block(&body, &mut exec);
                self.body = body;
            }
            ScanPhase::Output => {}
        }
    }

    fn apply_forces(&mut self) {
        let forced: Vec<(String, Value)> = self
            .forces
            .values
            .iter()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();
        for (name, value) in forced {
            self.state.values.insert(name, value);
        }
    }

    fn scan_phase_output(&self) -> Vec<String> {
        self.outputs
            .iter()
            .map(|name| {
                let value = self.state.get(name).cloned().unwrap_or(Value::Unknown);
                format!("{name} = {value}")
            })
            .collect()
    }

    /// Online "watch" snapshot: every declared scalar variable as `name = value`
    /// in source order. STRING values are rendered without surrounding quotes,
    /// the way an HMI / online Watch table displays them.
    pub fn watch(&self) -> Vec<String> {
        self.declared_order
            .iter()
            .map(|name| {
                let value = self.state.get(name).cloned().unwrap_or(Value::Unknown);
                format!("{name} = {}", display_watch(&value))
            })
            .collect()
    }
}

/// Render a value for the watch table the way an online monitor shows it:
/// strings without their quotes, REAL/TIME using the same CODESYS-style
/// formatting as `REAL_TO_STRING` / `TIME_TO_STRING`.
pub(crate) fn display_watch(value: &Value) -> String {
    match value {
        Value::Str(text) => text.clone(),
        Value::Real(real) => stdlib::real_to_string(*real),
        Value::Time(ms) => stdlib::time_to_string(*ms),
        other => other.to_string(),
    }
}
