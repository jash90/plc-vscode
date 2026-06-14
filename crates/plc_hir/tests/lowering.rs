use plc_hir::{BinaryOp, HirExpr, HirType, lower_source};

#[test]
fn lowers_program_vars_and_assignments() {
    let module = lower_source(
        "PROGRAM Main\nVAR\n    Count : INT;\nEND_VAR\nCount := Count + 1;\nEND_PROGRAM\n",
    );

    assert_eq!(module.programs.len(), 1);
    let program = &module.programs[0];
    assert_eq!(program.name, "Main");
    assert_eq!(program.vars.len(), 1);
    assert_eq!(program.vars[0].name, "Count");
    assert_eq!(program.vars[0].ty, HirType::Int);

    assert_eq!(program.body.len(), 1);
    let assign = &program.body[0];
    assert_eq!(assign.target, "Count");
    match &assign.value {
        HirExpr::Binary { op, lhs, rhs } => {
            assert_eq!(*op, BinaryOp::Add);
            assert_eq!(**lhs, HirExpr::Var("Count".to_owned()));
            assert_eq!(**rhs, HirExpr::Int(1));
        }
        other => panic!("expected binary add, got {other:?}"),
    }
}

#[test]
fn lowers_literal_operands() {
    let module =
        lower_source("PROGRAM Main\nVAR\n    Lamp : BOOL;\nEND_VAR\nLamp := TRUE;\nEND_PROGRAM\n");
    assert_eq!(module.programs[0].body[0].value, HirExpr::Bool(true));
}
