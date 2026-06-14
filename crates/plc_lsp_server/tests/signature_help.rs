use plc_lsp_server::{server_capabilities, signature_help_for_text};
use tower_lsp::lsp_types::{ParameterLabel, Position};

// Signature help protocol tests (PLC-56): assert the advertised capability and
// the textDocument/signatureHelp mapping for a standard function call and a
// user function-block instance call.

const PROGRAM_WITH_STANDARD_CALL: &str =
    "PROGRAM Main\nVAR\n    R : INT;\nEND_VAR\nR := MIN(1, 2);\nEND_PROGRAM\n";

const PROGRAM_WITH_FB_CALL: &str = concat!(
    "FUNCTION_BLOCK Counter\n",
    "VAR_INPUT\n",
    "    CU : BOOL;\n",
    "    PV : INT;\n",
    "END_VAR\n",
    "END_FUNCTION_BLOCK\n",
    "PROGRAM Main\n",
    "VAR\n",
    "    inst : Counter;\n",
    "END_VAR\n",
    "inst(CU := TRUE, PV := 10);\n",
    "END_PROGRAM\n",
);

#[test]
fn advertises_signature_help_support() {
    let capabilities = server_capabilities();
    assert!(capabilities.signature_help_provider.is_some());
}

#[test]
fn lsp_server_maps_signature_help_for_standard_function_call() {
    let help = signature_help_for_text(
        "file:///main.st",
        1,
        PROGRAM_WITH_STANDARD_CALL,
        Position {
            line: 4,
            character: 9,
        },
    )
    .expect("signature help for MIN");

    assert_eq!(help.active_signature, Some(0));
    assert_eq!(help.signatures.len(), 1);

    let signature = &help.signatures[0];
    assert_eq!(signature.label, "MIN(IN1 : ANY_NUM; IN2 : ANY_NUM)");
    assert_eq!(signature.active_parameter, Some(0));

    let parameters = signature
        .parameters
        .as_ref()
        .expect("parameters for MIN signature");
    assert_eq!(parameters.len(), 2);
    match &parameters[0].label {
        ParameterLabel::Simple(label) => assert_eq!(label, "IN1 : ANY_NUM"),
        other => panic!("unexpected parameter label: {other:?}"),
    }
}

#[test]
fn lsp_server_maps_signature_help_for_function_block_call() {
    let help = signature_help_for_text(
        "file:///main.st",
        1,
        PROGRAM_WITH_FB_CALL,
        Position {
            line: 10,
            character: 5,
        },
    )
    .expect("signature help for Counter instance");

    assert_eq!(help.signatures.len(), 1);
    let signature = &help.signatures[0];
    assert_eq!(signature.label, "Counter(CU : BOOL; PV : INT)");

    let parameters = signature
        .parameters
        .as_ref()
        .expect("parameters for Counter signature");
    assert_eq!(parameters.len(), 2);
    match &parameters[1].label {
        ParameterLabel::Simple(label) => assert_eq!(label, "PV : INT"),
        other => panic!("unexpected parameter label: {other:?}"),
    }
}
