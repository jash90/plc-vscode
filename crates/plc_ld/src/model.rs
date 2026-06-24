//! Ladder Diagram (LD) model — the serializable representation of a `.ld` file.
//!
//! The model is a tree of rungs, each containing a left-to-right sequence of
//! elements connected in series (AND) or parallel (OR) branches.  The structure
//! mirrors IEC 61131-3 LD: contacts (NO/NC), coils (normal/SET/RESET), and
//! function-block invocations (timers, counters, edge detectors).
//!
//! The model is serde-serializable so the VS Code custom editor can read/write
//! `.ld` files as JSON, and the CLI can emit power-flow results as JSON.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Top-level program
// ---------------------------------------------------------------------------

/// A complete Ladder Diagram program.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LdProgram {
    pub name: String,
    pub rungs: Vec<Rung>,
}

/// A single rung — one horizontal line from left rail to right rail.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rung {
    /// A rung is a parallel composition of series branches (OR of branches).
    pub branches: Vec<SeriesBranch>,
    /// The output element(s) driven by the rung logic.
    pub outputs: Vec<OutputElement>,
}

/// A series branch — a sequence of elements connected left-to-right (AND).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeriesBranch {
    pub elements: Vec<ContactElement>,
}

// ---------------------------------------------------------------------------
// Input elements (contacts)
// ---------------------------------------------------------------------------

/// A contact element in a series branch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContactElement {
    pub name: String,
    /// Normally-open (false) or normally-closed (true).
    pub negated: bool,
}

// ---------------------------------------------------------------------------
// Output elements (coils + blocks)
// ---------------------------------------------------------------------------

/// An output element at the right end of a rung.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum OutputElement {
    /// Normal coil `( )` — assigns the rung result to the variable.
    #[serde(rename = "coil")]
    Coil {
        name: String,
        variant: CoilVariant,
    },
    /// Function-block invocation (TON, TOF, TP, CTU, CTD, R_TRIG, F_TRIG, …).
    #[serde(rename = "block")]
    Block {
        fb_type: String,
        instance: String,
        /// Named input arguments (e.g. `IN`, `PT`).
        inputs: Vec<BlockArg>,
        /// Named output references (e.g. `Q` → variable name).
        outputs: Vec<BlockArg>,
    },
}

/// Coil variant — normal assignment, SET, or RESET.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CoilVariant {
    /// `( )` — standard assignment.
    Normal,
    /// `(S)` — SET: force TRUE when energized.
    Set,
    /// `(R)` — RESET: force FALSE when energized.
    Reset,
}

/// A named argument for a function-block invocation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockArg {
    pub name: String,
    /// The variable name or literal value connected to this pin.
    pub value: String,
}

// ---------------------------------------------------------------------------
// Power-flow result (for visualization)
// ---------------------------------------------------------------------------

/// The result of evaluating power flow on a program — which elements are
/// energized given the current variable state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PowerFlowResult {
    pub rungs: Vec<RungPowerFlow>,
}

/// Power-flow state for a single rung.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RungPowerFlow {
    /// Whether each parallel branch (by index) is energized.
    pub branch_energized: Vec<bool>,
    /// Whether each output element (by index) is energized.
    pub output_energized: Vec<bool>,
    /// The overall rung result (OR of all branches).
    pub rung_result: bool,
}

impl LdProgram {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            rungs: Vec::new(),
        }
    }

    /// Collect every variable name referenced anywhere in the program
    /// (contacts, coils, block inputs/outputs). Used for auto-declaring VARs.
    pub fn all_variables(&self) -> Vec<String> {
        let mut vars = std::collections::BTreeSet::new();
        for rung in &self.rungs {
            for branch in &rung.branches {
                for contact in &branch.elements {
                    vars.insert(contact.name.clone());
                }
            }
            for output in &rung.outputs {
                match output {
                    OutputElement::Coil { name, .. } => {
                        vars.insert(name.clone());
                    }
                    OutputElement::Block { inputs, outputs, .. } => {
                        for arg in inputs {
                            vars.insert(arg.value.clone());
                        }
                        for arg in outputs {
                            vars.insert(arg.value.clone());
                        }
                    }
                }
            }
        }
        vars.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_program_serializes_round_trip() {
        let program = LdProgram {
            name: "Motor".to_owned(),
            rungs: vec![Rung {
                branches: vec![SeriesBranch {
                    elements: vec![
                        ContactElement {
                            name: "Start".to_owned(),
                            negated: false,
                        },
                        ContactElement {
                            name: "Stop".to_owned(),
                            negated: true,
                        },
                    ],
                }],
                outputs: vec![OutputElement::Coil {
                    name: "Motor".to_owned(),
                    variant: CoilVariant::Normal,
                }],
            }],
        };

        let json = serde_json::to_string_pretty(&program).unwrap();
        let back: LdProgram = serde_json::from_str(&json).unwrap();
        assert_eq!(program, back);
    }

    #[test]
    fn block_rung_serializes_round_trip() {
        let program = LdProgram {
            name: "Timer".to_owned(),
            rungs: vec![Rung {
                branches: vec![SeriesBranch {
                    elements: vec![ContactElement {
                        name: "Start".to_owned(),
                        negated: false,
                    }],
                }],
                outputs: vec![OutputElement::Block {
                    fb_type: "TON".to_owned(),
                    instance: "Delay".to_owned(),
                    inputs: vec![
                        BlockArg {
                            name: "IN".to_owned(),
                            value: "Start".to_owned(),
                        },
                        BlockArg {
                            name: "PT".to_owned(),
                            value: "T#2s".to_owned(),
                        },
                    ],
                    outputs: vec![BlockArg {
                        name: "Q".to_owned(),
                        value: "Done".to_owned(),
                    }],
                }],
            }],
        };

        let json = serde_json::to_string_pretty(&program).unwrap();
        let back: LdProgram = serde_json::from_str(&json).unwrap();
        assert_eq!(program, back);
    }

    #[test]
    fn collects_all_variable_names() {
        let program = LdProgram {
            name: "Test".to_owned(),
            rungs: vec![
                Rung {
                    branches: vec![SeriesBranch {
                        elements: vec![
                            ContactElement {
                                name: "A".to_owned(),
                                negated: false,
                            },
                            ContactElement {
                                name: "B".to_owned(),
                                negated: true,
                            },
                        ],
                    }],
                    outputs: vec![OutputElement::Coil {
                        name: "C".to_owned(),
                        variant: CoilVariant::Normal,
                    }],
                },
                Rung {
                    branches: vec![SeriesBranch {
                        elements: vec![ContactElement {
                            name: "C".to_owned(),
                            negated: false,
                        }],
                    }],
                    outputs: vec![OutputElement::Block {
                        fb_type: "TON".to_owned(),
                        instance: "Delay".to_owned(),
                        inputs: vec![BlockArg {
                            name: "IN".to_owned(),
                            value: "C".to_owned(),
                        }],
                        outputs: vec![BlockArg {
                            name: "Q".to_owned(),
                            value: "Out".to_owned(),
                        }],
                    }],
                },
            ],
        };

        let vars = program.all_variables();
        assert!(vars.contains(&"A".to_owned()));
        assert!(vars.contains(&"B".to_owned()));
        assert!(vars.contains(&"C".to_owned()));
        assert!(vars.contains(&"Out".to_owned()));
        // Instance names are not in the variable list (they are FB instances).
        // "Delay" appears as a block arg value? No — it's the instance field.
        assert!(!vars.contains(&"Delay".to_owned()));
    }
}
