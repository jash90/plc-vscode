//! Deterministic PLC scan-cycle runtime for PLC VS Code.
//!
//! The runtime models the classic PLC execution loop — input scan, logic scan,
//! output scan — over retained variable state. Programs are taken from
//! `plc_syntax` parse output and executed deterministically so tests and the
//! simulator UI observe identical behavior on every run.

use std::collections::HashMap;

use plc_syntax::{StatementKind, parse_source};

mod value;

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
}

/// One assignment extracted from the parsed program (target := expression).
#[derive(Debug, Clone)]
struct Assignment {
    target: String,
    expression: String,
}

/// Deterministic scan-cycle runtime over a single Structured Text program.
#[derive(Debug, Clone)]
pub struct Runtime {
    program: Vec<Assignment>,
    state: VariableTable,
    pending_inputs: VariableTable,
    outputs: Vec<String>,
    scan_count: u64,
}

impl Runtime {
    /// Build a runtime from Structured Text source, retaining declared outputs.
    pub fn from_source(text: &str) -> Self {
        let parse = parse_source(text);
        let mut program = Vec::new();
        let mut outputs = Vec::new();
        let mut state = VariableTable::default();

        for unit in parse.units() {
            for block in &unit.declaration_blocks {
                if block.kind == plc_syntax::VarBlockKind::Output {
                    for declaration in &block.declarations {
                        outputs.push(declaration.name.clone());
                    }
                }
                // Cold-start initialization: declared variables take their
                // initializer if present, otherwise the type default.
                for declaration in &block.declarations {
                    let value = declaration
                        .initializer
                        .as_deref()
                        .and_then(Value::parse_literal)
                        .unwrap_or_else(|| Value::type_default(&declaration.type_name));
                    state.set(&declaration.name, value);
                }
            }
            for statement in &unit.statements {
                if statement.kind != StatementKind::Assignment {
                    continue;
                }
                if let (Some(target), Some(expression)) =
                    (statement.target.as_deref(), statement.expression.as_deref())
                {
                    program.push(Assignment {
                        target: target.to_owned(),
                        expression: expression.to_owned(),
                    });
                }
            }
        }

        Self {
            program,
            state,
            pending_inputs: VariableTable::default(),
            outputs,
            scan_count: 0,
        }
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
        self.scan_phase(ScanPhase::Input);
        self.scan_phase(ScanPhase::Logic);
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
                let program = self.program.clone();
                for assignment in &program {
                    let value = self.evaluate(&assignment.expression);
                    self.state.set(&assignment.target, value);
                }
            }
            ScanPhase::Output => {}
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

    /// Minimal deterministic expression evaluator: literals, variable
    /// references, and `a + b` / `a - b` over integers.
    fn evaluate(&self, expression: &str) -> Value {
        let trimmed = expression.trim();

        if let Some((left, right)) = split_binary(trimmed, '+') {
            return Value::add(self.operand(left), self.operand(right));
        }
        if let Some((left, right)) = split_binary(trimmed, '-') {
            return Value::sub(self.operand(left), self.operand(right));
        }
        self.operand(trimmed)
    }

    fn operand(&self, token: &str) -> Value {
        let token = token.trim();
        if let Some(value) = Value::parse_literal(token) {
            return value;
        }
        self.state.get(token).cloned().unwrap_or(Value::Unknown)
    }
}

/// Split `a <op> b` on the first top-level binary operator, if present.
fn split_binary(expression: &str, op: char) -> Option<(&str, &str)> {
    let index = expression.find(op)?;
    // Avoid treating a leading sign as a binary operator.
    if index == 0 {
        return None;
    }
    let (left, right) = expression.split_at(index);
    Some((left, &right[op.len_utf8()..]))
}
