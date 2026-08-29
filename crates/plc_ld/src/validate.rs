//! Structural validation for LD programs: stable `LD00xx` diagnostic codes
//! keyed by element id.
//!
//! [`validate`] is pure — no I/O, no lowering — so the editor, the CLI, and
//! the LSP diagnostics path all share one rule set. Codes:
//!
//! | Code | Severity | Meaning |
//! |---|---|---|
//! | `LD0001` | error | Empty rung (no branches) or empty branch (no contacts — lowers to a silent always-true) |
//! | `LD0002` | error | Duplicate function-block instance name |
//! | `LD0003` | error | Unknown function-block type (not in [`STANDARD_FB_TYPES`]) |
//! | `LD0004` | warning | Rung has no outputs |
//! | `LD0005` | warning | Unknown pin name for a known FB type |
//! | `LD0006` | error | Empty variable/instance name |

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::model::{LdProgram, OutputElement, Rung};

/// Diagnostic severity (crate-local; mapped to `plc_api` at the frontend).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LdSeverity {
    Error,
    Warning,
}

/// A structural LD diagnostic, keyed by rung index and element id when known.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LdDiagnostic {
    pub code: &'static str,
    pub severity: LdSeverity,
    /// Stable element id (PLC-107); absent for rung-level or id-less models.
    pub element_id: Option<String>,
    /// Zero-based rung index.
    pub rung: usize,
    pub message: String,
}

/// Function-block types the runtime can instantiate (case-sensitive: the
/// runtime dispatches on the exact name).
pub const STANDARD_FB_TYPES: &[&str] =
    &["TON", "TOF", "TP", "CTU", "CTD", "CTUD", "R_TRIG", "F_TRIG"];

/// Input and output pin names for a standard FB type, or `None` if unknown.
pub fn fb_pins(fb_type: &str) -> Option<(&'static [&'static str], &'static [&'static str])> {
    match fb_type {
        "TON" | "TOF" | "TP" => Some((&["IN", "PT"], &["Q", "ET"])),
        "CTU" => Some((&["CU", "RESET", "PV"], &["Q", "CV"])),
        "CTD" => Some((&["CD", "LOAD", "PV"], &["Q", "CV"])),
        "CTUD" => Some((&["CU", "CD", "RESET", "LOAD", "PV"], &["QU", "QD", "CV"])),
        "R_TRIG" | "F_TRIG" => Some((&["CLK"], &["Q"])),
        _ => None,
    }
}

/// Validate the structural integrity of an [`LdProgram`].
///
/// Order is deterministic: rules run per rung (empty → outputs → elements),
/// then the cross-rung instance-uniqueness pass.
pub fn validate(program: &LdProgram) -> Vec<LdDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut instances: HashSet<String> = HashSet::new();

    for (rung_index, rung) in program.rungs.iter().enumerate() {
        validate_rung(rung, rung_index, &mut diagnostics, &mut instances);
    }

    diagnostics
}

fn validate_rung(
    rung: &Rung,
    rung_index: usize,
    diagnostics: &mut Vec<LdDiagnostic>,
    instances: &mut HashSet<String>,
) {
    if rung.branches.is_empty() {
        diagnostics.push(LdDiagnostic {
            code: "LD0001",
            severity: LdSeverity::Error,
            element_id: rung.id.clone(),
            rung: rung_index,
            message: "empty rung: no branches".to_owned(),
        });
    }
    for branch in &rung.branches {
        if branch.elements.is_empty() {
            diagnostics.push(LdDiagnostic {
                code: "LD0001",
                severity: LdSeverity::Error,
                element_id: rung.id.clone(),
                rung: rung_index,
                message: "empty branch: a contact-less branch lowers to a \
                          silent always-true path"
                    .to_owned(),
            });
        }
        for contact in &branch.elements {
            if contact.name.trim().is_empty() {
                diagnostics.push(LdDiagnostic {
                    code: "LD0006",
                    severity: LdSeverity::Error,
                    element_id: contact.id.clone(),
                    rung: rung_index,
                    message: "empty variable name on contact".to_owned(),
                });
            }
        }
    }

    if rung.outputs.is_empty() {
        diagnostics.push(LdDiagnostic {
            code: "LD0004",
            severity: LdSeverity::Warning,
            element_id: rung.id.clone(),
            rung: rung_index,
            message: "rung has no outputs (coil or block)".to_owned(),
        });
    }

    for output in &rung.outputs {
        validate_output(output, rung_index, diagnostics, instances);
    }
}

fn validate_output(
    output: &OutputElement,
    rung_index: usize,
    diagnostics: &mut Vec<LdDiagnostic>,
    instances: &mut HashSet<String>,
) {
    match output {
        OutputElement::Coil { id, name, .. } => {
            if name.trim().is_empty() {
                diagnostics.push(LdDiagnostic {
                    code: "LD0006",
                    severity: LdSeverity::Error,
                    element_id: id.clone(),
                    rung: rung_index,
                    message: "empty variable name on coil".to_owned(),
                });
            }
        }
        OutputElement::Block {
            id,
            fb_type,
            instance,
            inputs,
            outputs,
        } => {
            if instance.trim().is_empty() {
                diagnostics.push(LdDiagnostic {
                    code: "LD0006",
                    severity: LdSeverity::Error,
                    element_id: id.clone(),
                    rung: rung_index,
                    message: "empty instance name on block".to_owned(),
                });
            } else if !instances.insert(instance.clone()) {
                diagnostics.push(LdDiagnostic {
                    code: "LD0002",
                    severity: LdSeverity::Error,
                    element_id: id.clone(),
                    rung: rung_index,
                    message: format!("duplicate function-block instance '{instance}'"),
                });
            }

            if !STANDARD_FB_TYPES.contains(&fb_type.as_str()) {
                diagnostics.push(LdDiagnostic {
                    code: "LD0003",
                    severity: LdSeverity::Error,
                    element_id: id.clone(),
                    rung: rung_index,
                    message: format!(
                        "unknown function-block type '{fb_type}' (known: {})",
                        STANDARD_FB_TYPES.join(", ")
                    ),
                });
            } else if let Some((input_pins, output_pins)) = fb_pins(fb_type) {
                for arg in inputs.iter().chain(outputs) {
                    if !input_pins.contains(&arg.name.as_str())
                        && !output_pins.contains(&arg.name.as_str())
                    {
                        diagnostics.push(LdDiagnostic {
                            code: "LD0005",
                            severity: LdSeverity::Warning,
                            element_id: id.clone(),
                            rung: rung_index,
                            message: format!(
                                "unknown pin '{}' for {} (inputs: {}; outputs: {})",
                                arg.name,
                                fb_type,
                                input_pins.join(", "),
                                output_pins.join(", ")
                            ),
                        });
                    }
                }
            }

            for arg in inputs.iter().chain(outputs) {
                if arg.value.trim().is_empty() {
                    diagnostics.push(LdDiagnostic {
                        code: "LD0006",
                        severity: LdSeverity::Error,
                        element_id: id.clone(),
                        rung: rung_index,
                        message: format!("empty value on pin '{}' of '{instance}'", arg.name),
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fb_pin_tables_cover_the_catalog() {
        for fb_type in STANDARD_FB_TYPES {
            assert!(fb_pins(fb_type).is_some(), "{fb_type} lacks a pin table");
        }
        assert!(fb_pins("MAGIC").is_none());
    }
}
