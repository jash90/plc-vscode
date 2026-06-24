//! HIR lowering coverage derived from `PRG_Test_ST.st`. The HIR now models
//! assignment bodies with a full operator set (`+`/`-`/`*`/`/`/`MOD`,
//! `AND`/`OR`/`XOR`, comparisons, `NOT`, and function calls).

use plc_hir::{BinaryOp, HirCallArg, HirExpr, HirPouKind, HirType, UnaryOp, lower_expression, lower_source};

const FIXTURE: &str = include_str!("fixtures/prg_test_st.st");

fn var(name: &str) -> HirExpr {
    HirExpr::Var(name.to_owned())
}

fn binary(op: BinaryOp, lhs: HirExpr, rhs: HirExpr) -> HirExpr {
    HirExpr::Binary {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

#[test]
fn addition_lowers_to_binary_add() {
    assert_eq!(
        lower_expression("iA + iB"),
        binary(BinaryOp::Add, var("iA"), var("iB"))
    );
}

#[test]
fn subtraction_lowers_to_binary_sub() {
    assert_eq!(
        lower_expression("iB - iA"),
        binary(BinaryOp::Sub, var("iB"), var("iA"))
    );
}

#[test]
fn multiplication_lowers_to_binary_mul() {
    assert_eq!(
        lower_expression("iA * iB"),
        binary(BinaryOp::Mul, var("iA"), var("iB"))
    );
}

#[test]
fn mod_lowers_to_binary_mod() {
    assert_eq!(
        lower_expression("17 MOD 5"),
        binary(BinaryOp::Mod, HirExpr::Int(17), HirExpr::Int(5))
    );
}

#[test]
fn comparison_lowers_to_binary_lt() {
    assert_eq!(
        lower_expression("iA < iB"),
        binary(BinaryOp::Lt, var("iA"), var("iB"))
    );
}

#[test]
fn division_lowers_to_binary_div() {
    assert_eq!(
        lower_expression("rA / rB"),
        binary(BinaryOp::Div, var("rA"), var("rB"))
    );
}

#[test]
fn function_calls_lower_to_call_expr() {
    assert_eq!(
        lower_expression("SHL(wA, 4)"),
        HirExpr::Call {
            name: "SHL".to_owned(),
            args: vec![
                HirCallArg { name: None, value: var("wA") },
                HirCallArg { name: None, value: HirExpr::Int(4) },
            ],
        }
    );
}

#[test]
fn boolean_operators_lower_correctly() {
    assert_eq!(
        lower_expression("A AND B"),
        binary(BinaryOp::And, var("A"), var("B"))
    );
    assert_eq!(
        lower_expression("A OR B"),
        binary(BinaryOp::Or, var("A"), var("B"))
    );
}

#[test]
fn not_lowers_to_unary_not() {
    assert_eq!(
        lower_expression("NOT A"),
        HirExpr::Unary {
            op: UnaryOp::Not,
            expr: Box::new(var("A")),
        }
    );
}

#[test]
fn literal_operands_lower_to_typed_exprs() {
    assert_eq!(lower_expression("2"), HirExpr::Int(2));
    assert_eq!(lower_expression("1"), HirExpr::Int(1));
    assert_eq!(lower_expression("7.0"), HirExpr::Real(7.0));
    assert_eq!(lower_expression("'dwa'"), HirExpr::Str("dwa".to_owned()));
    assert_eq!(lower_expression("FALSE"), HirExpr::Bool(false));
    assert_eq!(lower_expression("TRUE"), HirExpr::Bool(true));
}

#[test]
fn hir_type_from_name_divergences_used_by_the_file() {
    assert_eq!(HirType::from_name("DINT"), HirType::Int);
    assert_eq!(HirType::from_name("INT"), HirType::Int);
    assert_eq!(HirType::from_name("REAL"), HirType::Real);
    assert_eq!(HirType::from_name("STRING"), HirType::Str);
    assert_eq!(HirType::from_name("TIME"), HirType::Time);
    // Bit-strings and FB instance types are not modeled by the HIR type set.
    assert_eq!(HirType::from_name("WORD"), HirType::Unknown);
    assert_eq!(HirType::from_name("TON"), HirType::Unknown);
}

#[test]
fn lowers_program_name_and_kind() {
    let module = lower_source(FIXTURE);
    assert_eq!(module.programs.len(), 1);
    assert_eq!(module.programs[0].name, "PRG_Test");
    assert_eq!(module.programs[0].kind, HirPouKind::Program);
}

#[test]
fn lowers_var_types_for_mixed_declarations() {
    let src = "PROGRAM P\nVAR\n iA:INT; rA:REAL; xA:BOOL; sImie:STRING[20]; tA:TIME; wA:WORD; fbTON:TON;\nEND_VAR\nEND_PROGRAM\n";
    let module = lower_source(src);
    let vars = &module.programs[0].vars;
    let ty = |name: &str| vars.iter().find(|v| v.name == name).unwrap().ty;
    assert_eq!(ty("iA"), HirType::Int);
    assert_eq!(ty("rA"), HirType::Real);
    assert_eq!(ty("xA"), HirType::Bool);
    assert_eq!(ty("sImie"), HirType::Str);
    assert_eq!(ty("tA"), HirType::Time);
    assert_eq!(ty("wA"), HirType::Unknown);
    assert_eq!(ty("fbTON"), HirType::Unknown);
}

#[test]
fn fb_call_statements_are_not_lowered_into_body() {
    let src = "PROGRAM P\nVAR\n fbTON:TON; xWejscie:BOOL;\nEND_VAR\nfbTON(IN := xWejscie, PT := T#2s);\nEND_PROGRAM\n";
    assert!(lower_source(src).programs[0].body.is_empty());
}

#[test]
fn fixture_body_holds_assignments_including_known_targets() {
    let module = lower_source(FIXTURE);
    let body = &module.programs[0].body;
    assert!(
        body.len() >= 30,
        "expected many assignments, got {}",
        body.len()
    );
    assert!(body.iter().any(|a| a.target == "sLog01"));
    assert!(body.iter().any(|a| a.target == "iWynik"));
}

#[test]
fn first_arithmetic_assignment_lowers_to_add() {
    let src = "PROGRAM P\nVAR\n iA:INT:=2; iB:INT:=3; iWynik:INT;\nEND_VAR\niWynik := iA + iB;\nEND_PROGRAM\n";
    let module = lower_source(src);
    let assign = &module.programs[0].body[0];
    assert_eq!(assign.target, "iWynik");
    assert_eq!(assign.value, binary(BinaryOp::Add, var("iA"), var("iB")));
}
