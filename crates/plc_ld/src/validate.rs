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
//! | `LD0006` | error | Empty variable/instance/pin value |
//! | `LD0007` | error | FB instance name collides with a variable name |

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

/// Function-block types the runtime can instantiate. Compared
/// case-insensitively — the runtime upcases type names before dispatch —
/// but the canonical spellings below are what editors should write.
pub const STANDARD_FB_TYPES: &[&str] =
    &["TON", "TOF", "TP", "CTU", "CTD", "CTUD", "R_TRIG", "F_TRIG"];

/// True when `fb_type` names a standard FB (case-insensitive, like the
/// runtime's dispatch).
pub fn is_standard_fb(fb_type: &str) -> bool {
    STANDARD_FB_TYPES
        .iter()
        .any(|known| known.eq_ignore_ascii_case(fb_type))
}

/// Input and output pin names for a standard FB type, or `None` if unknown.
pub fn fb_pins(fb_type: &str) -> Option<(&'static [&'static str], &'static [&'static str])> {
    match fb_type.trim().to_ascii_uppercase().as_str() {
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
/// with the cross-rung instance-uniqueness and instance-vs-variable passes.
pub fn validate(program: &LdProgram) -> Vec<LdDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut instances: HashSet<String> = HashSet::new();
    let variables: HashSet<String> = program
        .all_variables()
        .into_iter()
        .map(|name| name.to_ascii_lowercase())
        .collect();

    for (rung_index, rung) in program.rungs.iter().enumerate() {
        validate_rung(
            rung,
            rung_index,
            &mut diagnostics,
            &mut instances,
            &variables,
        );
    }

    diagnostics
}

fn validate_rung(
    rung: &Rung,
    rung_index: usize,
    diagnostics: &mut Vec<LdDiagnostic>,
    instances: &mut HashSet<String>,
    variables: &HashSet<String>,
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
        validate_output(output, rung_index, diagnostics, instances, variables);
    }
}

fn validate_output(
    output: &OutputElement,
    rung_index: usize,
    diagnostics: &mut Vec<LdDiagnostic>,
    instances: &mut HashSet<String>,
    variables: &HashSet<String>,
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
            } else if !instances.insert(instance.to_ascii_lowercase()) {
                diagnostics.push(LdDiagnostic {
                    code: "LD0002",
                    severity: LdSeverity::Error,
                    element_id: id.clone(),
                    rung: rung_index,
                    message: format!("duplicate function-block instance '{instance}'"),
                });
            } else if variables.contains(&instance.to_ascii_lowercase()) {
                diagnostics.push(LdDiagnostic {
                    code: "LD0007",
                    severity: LdSeverity::Error,
                    element_id: id.clone(),
                    rung: rung_index,
                    message: format!(
                        "function-block instance '{instance}' collides with a \
                         variable of the same name (would render duplicate VAR \
                         declarations)"
                    ),
                });
            }

            if !is_standard_fb(fb_type) {
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
                    if !is_known_pin(&arg.name, input_pins) && !is_known_pin(&arg.name, output_pins)
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

            // Output-pin values become assignment targets (`Done := Inst.Q;`),
            // so they must be identifiers — a literal would render an ST
            // statement the runtime silently skips.
            for arg in outputs {
                if !is_identifier(&arg.value) {
                    diagnostics.push(LdDiagnostic {
                        code: "LD0006",
                        severity: LdSeverity::Error,
                        element_id: id.clone(),
                        rung: rung_index,
                        message: format!(
                            "output pin '{}' must be wired to a variable name, \
                             got '{}'",
                            arg.name, arg.value
                        ),
                    });
                }
            }
        }
    }
}

/// True for IEC-style identifiers: letter/underscore, then alnum/underscore.
fn is_identifier(name: &str) -> bool {
    let mut chars = name.trim().chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Pin-name acceptance matching the runtime: case-insensitive, with the
/// `R`→RESET and `LD`→LOAD aliases the interpreter honours.
fn is_known_pin(name: &str, pins: &[&str]) -> bool {
    let upper = name.trim().to_ascii_uppercase();
    pins.iter().any(|pin| pin.eq_ignore_ascii_case(&upper))
        || match upper.as_str() {
            "R" => pins.contains(&"RESET"),
            "LD" => pins.contains(&"LOAD"),
            _ => false,
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

    #[test]
    fn fb_type_matching_is_case_insensitive_like_the_runtime() {
        assert!(is_standard_fb("ton"));
        assert!(is_standard_fb("Ton"));
        assert!(!is_standard_fb("MAGIC"));
        // fb_pins itself must resolve through case.
        assert!(fb_pins("ctu").is_some());
    }

    #[test]
    fn pin_aliases_match_runtime_acceptance() {
        let (ctu_inputs, _) = fb_pins("CTU").unwrap();
        assert!(is_known_pin("R", ctu_inputs), "R aliases RESET");
        assert!(is_known_pin("reset", ctu_inputs), "case-insensitive");
        assert!(!is_known_pin("PT", ctu_inputs), "PT is not a CTU pin");
    }
}
