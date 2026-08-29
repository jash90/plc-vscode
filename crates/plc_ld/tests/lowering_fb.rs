//! PLC-109 — LD lowering completeness: FB instances keep their named type
//! and block output pins lower to assignments (`Done := Timer.Q;`).

use plc_hir::{HirExpr, HirStmt, HirType};
use plc_ld::{
    BlockArg, ContactElement, LdProgram, OutputElement, Rung, SeriesBranch, lower_ld_program,
};

fn program_with_ton() -> LdProgram {
    let mut program = LdProgram::new("TimerTest");
    program.rungs.push(Rung {
        id: None,
        comment: None,
        branches: vec![SeriesBranch {
            elements: vec![ContactElement {
                id: None,
                name: "Start".to_owned(),
                negated: false,
            }],
        }],
        outputs: vec![OutputElement::Block {
            id: None,
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
    });
    program
}

#[test]
fn fb_instance_var_keeps_named_type() {
    let module = lower_ld_program(&program_with_ton());
    let vars = &module.programs[0].vars;
    let delay = vars
        .iter()
        .find(|v| v.name == "Delay")
        .expect("instance var");
    assert_eq!(delay.ty, HirType::Named("TON".to_owned()));
}

#[test]
fn block_outputs_lower_to_assignments() {
    let module = lower_ld_program(&program_with_ton());
    let statements = &module.programs[0].statements;

    // FbCall first, then the output assignment wired to the FB member.
    assert!(matches!(&statements[0], HirStmt::FbCall { instance, .. } if instance == "Delay"));
    match &statements[1] {
        HirStmt::Assign(assign) => {
            assert_eq!(assign.target, "Done");
            assert_eq!(assign.value, HirExpr::Var("Delay.Q".to_owned()));
        }
        other => panic!("expected output assignment, got {other:?}"),
    }
}

#[test]
fn output_variable_is_declared() {
    let module = lower_ld_program(&program_with_ton());
    let vars = &module.programs[0].vars;
    assert!(
        vars.iter()
            .any(|v| v.name == "Done" && v.ty == HirType::Bool)
    );
}
