//! Full LD program integration: load a `.ld` fixture, lower to HIR, and
//! evaluate power-flow over variable states.

use plc_ld::{
    CoilVariant, ContactElement, LdProgram, OutputElement, Rung, SeriesBranch,
    evaluate_power_flow, lower_ld_program, parse_ld_json, var_state_from_watch,
};

const MOTOR_LD: &str = include_str!("../../../tests/ld/motor_control.ld");

#[test]
fn fixture_parses_and_lowers() {
    let program = parse_ld_json(MOTOR_LD).expect("fixture should parse");
    assert_eq!(program.name, "MotorControl");
    assert_eq!(program.rungs.len(), 4, "expected 4 rungs");

    let module = lower_ld_program(&program);
    assert_eq!(module.programs.len(), 1);
    let prog = &module.programs[0];

    // Seal-in rung + timer rung + counter rung + reset rung = 4 statements.
    // (normal coil + FbCall + FbCall + Reset)
    assert_eq!(prog.statements.len(), 4);
}

#[test]
fn fixture_power_flow_idle_state() {
    let program = parse_ld_json(MOTOR_LD).expect("fixture should parse");

    // All variables FALSE -> nothing energized.
    let watch = vec![
        "Start = FALSE".to_owned(),
        "Motor = FALSE".to_owned(),
        "MotorRun = FALSE".to_owned(),
        "Done = FALSE".to_owned(),
        "Pulse = FALSE".to_owned(),
        "Reached = FALSE".to_owned(),
    ];
    let state = var_state_from_watch(&watch);
    let result = evaluate_power_flow(&program, &state);

    // No rung should be energized in idle state.
    for (i, rung) in result.rungs.iter().enumerate() {
        assert!(
            !rung.rung_result,
            "rung {i} should not be energized in idle state"
        );
    }
}

#[test]
fn fixture_power_flow_start_pressed() {
    let program = parse_ld_json(MOTOR_LD).expect("fixture should parse");

    let watch = vec![
        "Start = TRUE".to_owned(),
        "Motor = FALSE".to_owned(),
        "MotorRun = FALSE".to_owned(),
        "Done = FALSE".to_owned(),
        "Pulse = FALSE".to_owned(),
        "Reached = FALSE".to_owned(),
    ];
    let state = var_state_from_watch(&watch);
    let result = evaluate_power_flow(&program, &state);

    // Rung 0 (seal-in) should be energized: Start OR Motor, AND NOT Stop.
    assert!(
        result.rungs[0].rung_result,
        "seal-in rung should be energized when Start=TRUE"
    );
    // Rung 2 (counter) should not be energized: Pulse=FALSE.
    assert!(
        !result.rungs[2].rung_result,
        "counter rung should not be energized"
    );
}

#[test]
fn manual_program_lowers_and_evaluates() {
    // Build a program manually: (A AND NOT B) -> C
    let program = LdProgram {
        name: "Manual".to_owned(),
        rungs: vec![Rung {
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
        }],
    };

    let module = lower_ld_program(&program);
    assert_eq!(module.programs[0].statements.len(), 1);

    // Power flow: A=TRUE, B=FALSE -> energized.
    let mut state = std::collections::HashMap::new();
    state.insert("a".to_owned(), true);
    state.insert("b".to_owned(), false);
    let result = evaluate_power_flow(&program, &state);
    assert!(result.rungs[0].rung_result);
    assert!(result.rungs[0].output_energized[0]);
}
