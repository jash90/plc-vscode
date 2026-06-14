use plc_runtime::{BytecodeModule, Instruction};

fn sample_module() -> BytecodeModule {
    BytecodeModule::new(
        "Main",
        vec![
            Instruction::LoadVar("Count".to_owned()),
            Instruction::PushInt(1),
            Instruction::Add,
            Instruction::StoreVar("Count".to_owned()),
        ],
    )
}

#[test]
fn bytecode_round_trips_through_json() {
    let module = sample_module();
    let json = module.to_json();
    let restored = BytecodeModule::from_json(&json).expect("valid bytecode json");
    assert_eq!(module, restored);
}

#[test]
fn disassembly_is_indexed_and_mnemonic() {
    let listing = sample_module().disassemble();
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
