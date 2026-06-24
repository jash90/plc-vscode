//! Power-flow evaluation: determines which elements in a rung are energized
//! given the current state of variables.
//!
//! This mirrors how a real PLC evaluates a rung left-to-right: contacts pass
//! power if their variable state matches their type (NO = TRUE, NC = FALSE),
//! series contacts AND, parallel branches OR, and the result drives the coil.
//!
//! The output [`PowerFlowResult`] is serialized as JSON for the VS Code webview
//! to color elements green (energized) or gray (not energized).

use std::collections::HashMap;

use crate::model::{CoilVariant, ContactElement, LdProgram, OutputElement, Rung, RungPowerFlow};
use crate::PowerFlowResult;

/// Variable state: variable name (lowercased) → boolean value.
pub type VarState = HashMap<String, bool>;

/// Evaluate power flow for an entire program.
pub fn evaluate_power_flow(program: &LdProgram, state: &VarState) -> PowerFlowResult {
    let rungs: Vec<RungPowerFlow> = program
        .rungs
        .iter()
        .map(|rung| evaluate_rung(rung, state))
        .collect();
    PowerFlowResult { rungs }
}

/// Evaluate one rung.
fn evaluate_rung(rung: &Rung, state: &VarState) -> RungPowerFlow {
    // Evaluate each parallel branch.
    let branch_results: Vec<bool> = rung
        .branches
        .iter()
        .map(|branch| evaluate_series(branch.elements.iter(), state))
        .collect();

    let rung_result = branch_results.iter().any(|&b| b);

    let output_results: Vec<bool> = rung
        .outputs
        .iter()
        .map(|output| evaluate_output(output, rung_result, state))
        .collect();

    RungPowerFlow {
        branch_energized: branch_results,
        output_energized: output_results,
        rung_result,
    }
}

/// Evaluate a series of contacts (AND).
fn evaluate_series<'a>(
    contacts: impl Iterator<Item = &'a ContactElement>,
    state: &VarState,
) -> bool {
    contacts
        .map(|c| evaluate_contact(c, state))
        .fold(true, |acc, energized| acc && energized)
}

/// A contact is energized if the variable state matches the contact type:
/// NO passes when the variable is TRUE; NC passes when it is FALSE.
fn evaluate_contact(contact: &ContactElement, state: &VarState) -> bool {
    let value = state
        .get(&contact.name.to_ascii_lowercase())
        .copied()
        .unwrap_or(false);
    if contact.negated {
        !value
    } else {
        value
    }
}

/// Evaluate an output element: a normal coil is energized when the rung is
/// energized; SET/RESET coils and blocks are always "active" in the sense that
/// they are driven by the rung.
fn evaluate_output(
    output: &OutputElement,
    rung_result: bool,
    _state: &VarState,
) -> bool {
    match output {
        OutputElement::Coil { variant, .. } => match variant {
            CoilVariant::Normal => rung_result,
            // SET/RESET coils are "energized" (visually active) when the rung
            // has power — the actual set/reset happens in the runtime.
            CoilVariant::Set | CoilVariant::Reset => rung_result,
        },
        // Blocks are energized when their IN pin (rung logic) has power.
        OutputElement::Block { .. } => rung_result,
    }
}

/// Build a [`VarState`] from `name = value` watch strings (as produced by the
/// runtime's `watch()`). Non-boolean values are coerced: non-zero/non-empty →
/// true.
pub fn var_state_from_watch(watch: &[String]) -> VarState {
    let mut state = VarState::new();
    for line in watch {
        if let Some((name, value)) = line.split_once('=') {
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim();
            let bool_val = match value.to_ascii_uppercase().as_str() {
                "TRUE" => true,
                "FALSE" => false,
                _ => {
                    // Try to parse as a number — non-zero is true.
                    if let Ok(n) = value.parse::<i64>() {
                        n != 0
                    } else if let Ok(f) = value.parse::<f64>() {
                        f != 0.0
                    } else {
                        !value.is_empty()
                    }
                }
            };
            state.insert(name, bool_val);
        }
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn make_rung(contacts: &[(&str, bool)], output_name: &str) -> Rung {
        Rung {
            branches: vec![SeriesBranch {
                elements: contacts
                    .iter()
                    .map(|(name, negated)| ContactElement {
                        name: (*name).to_owned(),
                        negated: *negated,
                    })
                    .collect(),
            }],
            outputs: vec![OutputElement::Coil {
                name: output_name.to_owned(),
                variant: CoilVariant::Normal,
            }],
        }
    }

    #[test]
    fn no_contact_with_true_passes_power() {
        let rung = make_rung(&[("A", false)], "C");
        let mut state = VarState::new();
        state.insert("a".to_owned(), true);

        let result = evaluate_rung(&rung, &state);
        assert!(result.branch_energized[0]);
        assert!(result.output_energized[0]);
        assert!(result.rung_result);
    }

    #[test]
    fn no_contact_with_false_blocks_power() {
        let rung = make_rung(&[("A", false)], "C");
        let mut state = VarState::new();
        state.insert("a".to_owned(), false);

        let result = evaluate_rung(&rung, &state);
        assert!(!result.branch_energized[0]);
        assert!(!result.output_energized[0]);
        assert!(!result.rung_result);
    }

    #[test]
    fn nc_contact_passes_when_false() {
        let rung = make_rung(&[("Stop", true)], "C");
        let mut state = VarState::new();
        state.insert("stop".to_owned(), false);

        let result = evaluate_rung(&rung, &state);
        assert!(result.branch_energized[0], "NC passes when variable is FALSE");
    }

    #[test]
    fn nc_contact_blocks_when_true() {
        let rung = make_rung(&[("Stop", true)], "C");
        let mut state = VarState::new();
        state.insert("stop".to_owned(), true);

        let result = evaluate_rung(&rung, &state);
        assert!(!result.branch_energized[0], "NC blocks when variable is TRUE");
    }

    #[test]
    fn series_and_logic() {
        let rung = make_rung(&[("A", false), ("B", false)], "C");
        let mut state = VarState::new();
        state.insert("a".to_owned(), true);
        state.insert("b".to_owned(), false);

        let result = evaluate_rung(&rung, &state);
        assert!(!result.rung_result, "A AND FALSE = FALSE");

        state.insert("b".to_owned(), true);
        let result = evaluate_rung(&rung, &state);
        assert!(result.rung_result, "A AND TRUE = TRUE");
    }

    #[test]
    fn parallel_or_logic() {
        let rung = Rung {
            branches: vec![
                SeriesBranch {
                    elements: vec![ContactElement {
                        name: "A".to_owned(),
                        negated: false,
                    }],
                },
                SeriesBranch {
                    elements: vec![ContactElement {
                        name: "B".to_owned(),
                        negated: false,
                    }],
                },
            ],
            outputs: vec![OutputElement::Coil {
                name: "C".to_owned(),
                variant: CoilVariant::Normal,
            }],
        };

        let mut state = VarState::new();
        state.insert("a".to_owned(), false);
        state.insert("b".to_owned(), false);
        assert!(!evaluate_rung(&rung, &state).rung_result);

        state.insert("a".to_owned(), true);
        assert!(evaluate_rung(&rung, &state).rung_result);
    }

    #[test]
    fn var_state_from_watch_parses_booleans() {
        let watch = vec![
            "A = TRUE".to_owned(),
            "B = FALSE".to_owned(),
            "Count = 5".to_owned(),
            "Name = hello".to_owned(),
        ];
        let state = var_state_from_watch(&watch);
        assert_eq!(state.get("a"), Some(&true));
        assert_eq!(state.get("b"), Some(&false));
        assert_eq!(state.get("count"), Some(&true)); // non-zero = true
        assert_eq!(state.get("name"), Some(&true)); // non-empty = true
    }
}
