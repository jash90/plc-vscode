//! LD → HIR lowering: converts an [`LdProgram`] into a [`plc_hir::HirModule`].
//!
//! The lowering maps Ladder Diagram constructs to IEC 61131-3 ST equivalents:
//!
//! | LD construct | HIR / ST equivalent |
//! |---|---|
//! | Series contacts (AND) | `Binary { And, ... }` |
//! | Parallel branches (OR) | `Binary { Or, ... }` |
//! | Normally-closed contact (NC) | `Unary { Not, Var }` |
//! | Normal coil `( )` | `Assign` |
//! | SET coil `(S)` | `Set { target, value }` |
//! | RESET coil `(R)` | `Reset { target, value }` |
//! | FB block (TON, …) | `FbCall { instance, args }` |
//!
//! The resulting `HirModule` is rendered to ST text by the existing
//! `StFrontend::render`, so LD→ST conversion works through the IR hub without
//! any ST-specific code in this crate.

use plc_hir::{
    BinaryOp, HirCallArg, HirExpr, HirModule, HirPouKind, HirProgram, HirStmt, HirType, HirVar,
};

use crate::model::{
    CoilVariant, ContactElement, LdProgram, OutputElement, Rung, SeriesBranch,
};

/// Lower an [`LdProgram`] into a [`HirModule`] with a single PROGRAM POU.
///
/// Each rung produces one or more HIR statements (assignments, SET/RESET,
/// or function-block calls). Variables are auto-declared as `BOOL` unless
/// the block argument looks like a TIME literal.
pub fn lower_ld_program(program: &LdProgram) -> HirModule {
    let mut vars = collect_variables(program);
    let mut statements = Vec::new();

    for rung in &program.rungs {
        let rung_logic = lower_rung_logic(rung);
        for output in &rung.outputs {
            let stmt = lower_output(output, &rung_logic);
            statements.push(stmt);
        }
    }

    // Also ensure block instance variables are declared.
    for rung in &program.rungs {
        for output in &rung.outputs {
            if let OutputElement::Block {
                instance,
                fb_type,
                ..
            } = output
            {
                vars.push(HirVar {
                    name: instance.clone(),
                    ty: HirType::from_name(fb_type),
                });
            }
        }
    }

    HirModule {
        programs: vec![HirProgram {
            name: program.name.clone(),
            kind: HirPouKind::Program,
            vars,
            body: Vec::new(),
            statements,
        }],
    }
}

/// Build the boolean expression for a rung's contact logic (everything left of
/// the output). Parallel branches are OR'd; series contacts within a branch are
/// AND'd; normally-closed contacts are NOT'd.
fn lower_rung_logic(rung: &Rung) -> HirExpr {
    let branches: Vec<HirExpr> = rung
        .branches
        .iter()
        .map(|branch| lower_series_branch(branch))
        .collect();

    if branches.is_empty() {
        return HirExpr::Bool(false);
    }

    branches
        .into_iter()
        .reduce(|acc, branch| HirExpr::Binary {
            op: BinaryOp::Or,
            lhs: Box::new(acc),
            rhs: Box::new(branch),
        })
        .unwrap()
}

/// AND together all contacts in a series branch.
fn lower_series_branch(branch: &SeriesBranch) -> HirExpr {
    let contacts: Vec<HirExpr> = branch
        .elements
        .iter()
        .map(lower_contact)
        .collect();

    if contacts.is_empty() {
        return HirExpr::Bool(true);
    }

    contacts
        .into_iter()
        .reduce(|acc, contact| HirExpr::Binary {
            op: BinaryOp::And,
            lhs: Box::new(acc),
            rhs: Box::new(contact),
        })
        .unwrap()
}

/// Convert a contact: NO passes the variable through, NC negates it.
fn lower_contact(contact: &ContactElement) -> HirExpr {
    let var = HirExpr::Var(contact.name.clone());
    if contact.negated {
        HirExpr::Unary {
            op: plc_hir::UnaryOp::Not,
            expr: Box::new(var),
        }
    } else {
        var
    }
}

/// Convert an output element to a HIR statement, driven by the rung logic.
fn lower_output(output: &OutputElement, rung_logic: &HirExpr) -> HirStmt {
    match output {
        OutputElement::Coil { name, variant } => match variant {
            CoilVariant::Normal => HirStmt::Assign(plc_hir::HirAssign {
                target: name.clone(),
                value: rung_logic.clone(),
            }),
            CoilVariant::Set => HirStmt::Set {
                target: name.clone(),
                value: rung_logic.clone(),
            },
            CoilVariant::Reset => HirStmt::Reset {
                target: name.clone(),
                value: rung_logic.clone(),
            },
        },
        OutputElement::Block {
            instance,
            fb_type,
            inputs,
            ..
        } => {
            let args = inputs
                .iter()
                .map(|arg| {
                    // The IN pin of a timer/counter receives the rung logic,
                    // not the literal variable name. Other pins (PT, PV, etc.)
                    // pass through as variable references.
                    let value = if arg.name.eq_ignore_ascii_case("IN")
                        || arg.name.eq_ignore_ascii_case("CU")
                        || arg.name.eq_ignore_ascii_case("CD")
                    {
                        rung_logic.clone()
                    } else {
                        HirExpr::Var(arg.value.clone())
                    };
                    HirCallArg {
                        name: Some(arg.name.clone()),
                        value,
                    }
                })
                .collect();
            HirStmt::FbCall {
                instance: instance.clone(),
                fb_type: fb_type.clone(),
                args,
            }
        }
    }
}

/// Collect all variable declarations for the program.
fn collect_variables(program: &LdProgram) -> Vec<HirVar> {
    let mut vars = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for rung in &program.rungs {
        for branch in &rung.branches {
            for contact in &branch.elements {
                if seen.insert(contact.name.clone()) {
                    vars.push(HirVar {
                        name: contact.name.clone(),
                        ty: HirType::Bool,
                    });
                }
            }
        }
        for output in &rung.outputs {
            if let OutputElement::Coil { name, .. } = output {
                if seen.insert(name.clone()) {
                    vars.push(HirVar {
                        name: name.clone(),
                        ty: HirType::Bool,
                    });
                }
            }
            if let OutputElement::Block { outputs, .. } = output {
                for arg in outputs {
                    if seen.insert(arg.value.clone()) {
                        vars.push(HirVar {
                            name: arg.value.clone(),
                            ty: HirType::Bool,
                        });
                    }
                }
            }
        }
    }

    vars
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    #[test]
    fn simple_and_lowers_to_assignment() {
        // (A AND B) → C
        let program = LdProgram {
            name: "Test".to_owned(),
            rungs: vec![Rung {
                branches: vec![SeriesBranch {
                    elements: vec![
                        ContactElement {
                            name: "A".to_owned(),
                            negated: false,
                        },
                        ContactElement {
                            name: "B".to_owned(),
                            negated: false,
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
        assert_eq!(module.programs.len(), 1);
        let stmts = &module.programs[0].statements;
        assert_eq!(stmts.len(), 1);

        // Should be Assign(C, A AND B)
        match &stmts[0] {
            HirStmt::Assign(assign) => {
                assert_eq!(assign.target, "C");
                // A AND B
                match &assign.value {
                    HirExpr::Binary { op, .. } => {
                        assert_eq!(*op, BinaryOp::And);
                    }
                    _ => panic!("expected binary AND, got {:?}", assign.value),
                }
            }
            _ => panic!("expected Assign, got {:?}", stmts[0]),
        }
    }

    #[test]
    fn nc_contact_lowers_to_not() {
        // (A AND NOT B) → C
        let program = LdProgram {
            name: "Test".to_owned(),
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
        let stmts = &module.programs[0].statements;
        // The AND of A and NOT B — check the NOT is there.
        match &stmts[0] {
            HirStmt::Assign(assign) => match &assign.value {
                HirExpr::Binary { rhs, .. } => match rhs.as_ref() {
                    HirExpr::Unary { op, .. } => {
                        assert_eq!(*op, plc_hir::UnaryOp::Not);
                    }
                    _ => panic!("expected NOT on rhs, got {:?}", rhs),
                },
                _ => panic!("expected binary, got {:?}", assign.value),
            },
            _ => panic!("expected Assign"),
        }
    }

    #[test]
    fn parallel_branches_lowers_to_or() {
        // (A OR B) → C
        let program = LdProgram {
            name: "Test".to_owned(),
            rungs: vec![Rung {
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
            }],
        };

        let module = lower_ld_program(&program);
        let stmts = &module.programs[0].statements;
        match &stmts[0] {
            HirStmt::Assign(assign) => match &assign.value {
                HirExpr::Binary { op, .. } => assert_eq!(*op, BinaryOp::Or),
                _ => panic!("expected OR, got {:?}", assign.value),
            },
            _ => panic!("expected Assign"),
        }
    }

    #[test]
    fn set_coil_lowers_to_set_stmt() {
        let program = LdProgram {
            name: "Test".to_owned(),
            rungs: vec![Rung {
                branches: vec![SeriesBranch {
                    elements: vec![ContactElement {
                        name: "Start".to_owned(),
                        negated: false,
                    }],
                }],
                outputs: vec![OutputElement::Coil {
                    name: "Latched".to_owned(),
                    variant: CoilVariant::Set,
                }],
            }],
        };

        let module = lower_ld_program(&program);
        assert!(matches!(
            module.programs[0].statements[0],
            HirStmt::Set { .. }
        ));
    }

    #[test]
    fn timer_block_lowers_to_fbcall() {
        let program = LdProgram {
            name: "Test".to_owned(),
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

        let module = lower_ld_program(&program);
        match &module.programs[0].statements[0] {
            HirStmt::FbCall {
                instance, args, ..
            } => {
                assert_eq!(instance, "Delay");
                assert_eq!(args.len(), 2);
                assert_eq!(args[0].name.as_deref(), Some("IN"));
            }
            _ => panic!("expected FbCall"),
        }
    }

    #[test]
    fn variables_auto_declared() {
        let program = LdProgram {
            name: "Test".to_owned(),
            rungs: vec![Rung {
                branches: vec![SeriesBranch {
                    elements: vec![
                        ContactElement {
                            name: "A".to_owned(),
                            negated: false,
                        },
                        ContactElement {
                            name: "B".to_owned(),
                            negated: false,
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
        let var_names: Vec<&str> =
            module.programs[0].vars.iter().map(|v| v.name.as_str()).collect();
        assert!(var_names.contains(&"A"));
        assert!(var_names.contains(&"B"));
        assert!(var_names.contains(&"C"));
    }
}
