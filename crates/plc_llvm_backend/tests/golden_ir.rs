use plc_llvm_backend::emit_ir_from_source;

#[test]
fn lowers_simple_program_to_llvm_ir() {
    let ir = emit_ir_from_source(
        "PROGRAM Main\nVAR\n    Count : INT;\nEND_VAR\nCount := Count + 1;\nEND_PROGRAM\n",
    );

    // Golden expectations on the emitted IR shape.
    assert!(ir.contains("define void @Main"), "IR was:\n{ir}");
    assert!(ir.contains("alloca i64"), "IR was:\n{ir}");
    assert!(ir.contains("add i64"), "IR was:\n{ir}");
    assert!(ir.contains("ret void"), "IR was:\n{ir}");
}

#[test]
fn lowers_function_block_state_to_struct_and_run_function() {
    let ir = emit_ir_from_source(
        "FUNCTION_BLOCK Counter\nVAR\n    CV : INT;\nEND_VAR\nCV := CV + 1;\nEND_FUNCTION_BLOCK\n",
    );

    // State struct with one field and a run function over a self pointer.
    assert!(ir.contains("%FB_Counter = type { i64 }"), "IR was:\n{ir}");
    assert!(ir.contains("define void @Counter_run(ptr"), "IR was:\n{ir}");
    assert!(ir.contains("getelementptr"), "IR was:\n{ir}");
    assert!(ir.contains("add i64"), "IR was:\n{ir}");
}

#[test]
fn lowers_multiple_programs() {
    let ir = emit_ir_from_source(
        "PROGRAM A\nVAR\n    X : INT;\nEND_VAR\nX := 1;\nEND_PROGRAM\nPROGRAM B\nVAR\n    Y : INT;\nEND_VAR\nY := X - 1;\nEND_PROGRAM\n",
    );

    assert!(ir.contains("define void @A"), "IR was:\n{ir}");
    assert!(ir.contains("define void @B"), "IR was:\n{ir}");
    assert!(ir.contains("sub i64"), "IR was:\n{ir}");
}
