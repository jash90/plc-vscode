//! Golden tests for HIR -> bytecode lowering (VM-side completion of PLC-45).

use plc_hir::lower_source;
use plc_runtime::lower_module;

#[test]
fn lowers_counter_program_to_golden_bytecode() {
    let hir = lower_source(
        "PROGRAM Main\nVAR\n    Count : INT;\nEND_VAR\nCount := Count + 1;\nEND_PROGRAM\n",
    );
    let modules = lower_module(&hir);
    assert_eq!(modules.len(), 1);

    let listing = modules[0].disassemble();
    assert_eq!(
        listing,
        vec![
            "0000  LOAD_VAR Count".to_owned(),
            "0001  PUSH_INT 1".to_owned(),
            "0002  ADD".to_owned(),
            "0003  STORE_VAR Count".to_owned(),
        ]
    );
}

#[test]
fn lowered_bytecode_round_trips_through_json() {
    let hir = lower_source("PROGRAM Main\nVAR\n    X : INT;\nEND_VAR\nX := X - 2;\nEND_PROGRAM\n");
    let module = &lower_module(&hir)[0];
    let restored = plc_runtime::BytecodeModule::from_json(&module.to_json()).expect("valid json");
    assert_eq!(*module, restored);
    assert!(module.disassemble().iter().any(|line| line.contains("SUB")));
}
