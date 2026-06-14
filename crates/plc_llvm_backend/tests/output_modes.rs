use plc_hir::lower_source;
use plc_llvm_backend::{OutputMode, compile};

const SOURCE: &str =
    "PROGRAM Main\nVAR\n    Count : INT;\nEND_VAR\nCount := Count + 1;\nEND_PROGRAM\n";

#[test]
fn llvm_ir_mode_emits_textual_ir() {
    let module = lower_source(SOURCE);
    let bytes = compile(&module, OutputMode::LlvmIr).expect("ir emit");
    let text = String::from_utf8(bytes).expect("utf8 ir");
    assert!(text.contains("define void @Main"));
}

#[test]
fn assembly_mode_emits_text() {
    let module = lower_source(SOURCE);
    let bytes = compile(&module, OutputMode::Assembly).expect("asm emit");
    assert!(!bytes.is_empty());
    // Assembly references the symbol name.
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("Main"));
}

#[test]
fn object_and_linkable_modes_emit_machine_code() {
    let module = lower_source(SOURCE);
    for mode in [
        OutputMode::Object,
        OutputMode::StaticLibrary,
        OutputMode::SharedLibrary,
        OutputMode::Executable,
    ] {
        let bytes = compile(&module, mode).unwrap_or_else(|e| panic!("{mode:?}: {e}"));
        assert!(!bytes.is_empty(), "{mode:?} produced no bytes");
    }
}
