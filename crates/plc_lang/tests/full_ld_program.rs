//! Full LD program with timers, counters, and SET/RESET coils — conversion to
//! ST and power-flow evaluation.

use plc_api::SourceDocument;
use plc_lang::LanguageRegistry;

/// LD program with a TON timer block and a SET coil.
const TIMER_LD: &str = r#"{
    "name": "TimerControl",
    "rungs": [
        {
            "branches": [
                {
                    "elements": [{ "name": "Start", "negated": false }]
                }
            ],
            "outputs": [
                {
                    "kind": "block",
                    "fb_type": "TON",
                    "instance": "Delay",
                    "inputs": [
                        { "name": "IN", "value": "Start" },
                        { "name": "PT", "value": "T#2s" }
                    ],
                    "outputs": [
                        { "name": "Q", "value": "Done" }
                    ]
                }
            ]
        },
        {
            "branches": [
                {
                    "elements": [{ "name": "Done", "negated": false }]
                }
            ],
            "outputs": [
                {
                    "kind": "coil",
                    "name": "Output",
                    "variant": "set"
                }
            ]
        }
    ]
}"#;

#[test]
fn timer_and_set_coil_convert_to_st() {
    let registry = LanguageRegistry::with_builtins();
    let document = SourceDocument::new("file:///timer.ld", 0, TIMER_LD);

    let out = registry.convert("ld", "st", &document);
    assert!(out.error.is_none(), "conversion error: {:?}", out.error);

    let st = &out.text;
    // Should contain the TON instance call.
    assert!(
        st.contains("Delay("),
        "ST should contain TON call, was:\n{st}"
    );
    // Should contain IN and PT arguments.
    assert!(st.contains("IN"), "ST was:\n{st}");
    assert!(st.contains("PT"), "ST was:\n{st}");
    // Should contain the SET coil as an IF statement.
    assert!(
        st.contains("TRUE") || st.contains("Output"),
        "ST should contain SET coil logic, was:\n{st}"
    );
}

/// LD program with a CTU counter block.
const COUNTER_LD: &str = r#"{
    "name": "CounterTest",
    "rungs": [
        {
            "branches": [
                {
                    "elements": [{ "name": "Pulse", "negated": false }]
                }
            ],
            "outputs": [
                {
                    "kind": "block",
                    "fb_type": "CTU",
                    "instance": "Counter",
                    "inputs": [
                        { "name": "CU", "value": "Pulse" },
                        { "name": "PV", "value": "10" }
                    ],
                    "outputs": [
                        { "name": "Q", "value": "Reached" }
                    ]
                }
            ]
        }
    ]
}"#;

#[test]
fn counter_block_converts_to_st() {
    let registry = LanguageRegistry::with_builtins();
    let document = SourceDocument::new("file:///counter.ld", 0, COUNTER_LD);

    let out = registry.convert("ld", "st", &document);
    assert!(out.error.is_none(), "conversion error: {:?}", out.error);

    let st = &out.text;
    assert!(
        st.contains("Counter("),
        "ST should contain CTU call, was:\n{st}"
    );
    assert!(st.contains("CU"), "ST was:\n{st}");
    assert!(st.contains("PV"), "ST was:\n{st}");
}

/// LD program with RESET coil.
const RESET_LD: &str = r#"{
    "name": "ResetTest",
    "rungs": [
        {
            "branches": [
                {
                    "elements": [{ "name": "Stop", "negated": false }]
                }
            ],
            "outputs": [
                {
                    "kind": "coil",
                    "name": "Motor",
                    "variant": "reset"
                }
            ]
        }
    ]
}"#;

#[test]
fn reset_coil_converts_to_st() {
    let registry = LanguageRegistry::with_builtins();
    let document = SourceDocument::new("file:///reset.ld", 0, RESET_LD);

    let out = registry.convert("ld", "st", &document);
    assert!(out.error.is_none(), "conversion error: {:?}", out.error);

    let st = &out.text;
    assert!(
        st.contains("FALSE"),
        "RESET coil should produce := FALSE, was:\n{st}"
    );
    assert!(
        st.contains("Motor"),
        "ST should reference Motor, was:\n{st}"
    );
}

/// Complex program: motor start-stop with seal-in (latching).
/// Rung 1: (Start OR Motor) AND NOT Stop → Motor (normal coil)
/// Rung 2: Motor AND NOT Done → TON delay → Done
/// Rung 3: Done → Motor (RESET coil)
const COMPLEX_LD: &str = r#"{
    "name": "MotorControl",
    "rungs": [
        {
            "branches": [
                {
                    "elements": [{ "name": "Start", "negated": false }]
                },
                {
                    "elements": [{ "name": "Motor", "negated": false }]
                }
            ],
            "outputs": [
                {
                    "kind": "coil",
                    "name": "MotorRun",
                    "variant": "normal"
                }
            ]
        },
        {
            "branches": [
                {
                    "elements": [
                        { "name": "MotorRun", "negated": false },
                        { "name": "Done", "negated": true }
                    ]
                }
            ],
            "outputs": [
                {
                    "kind": "block",
                    "fb_type": "TON",
                    "instance": "Timer",
                    "inputs": [
                        { "name": "IN", "value": "MotorRun" },
                        { "name": "PT", "value": "T#5s" }
                    ],
                    "outputs": [
                        { "name": "Q", "value": "Done" }
                    ]
                }
            ]
        }
    ]
}"#;

#[test]
fn complex_motor_control_converts_to_st() {
    let registry = LanguageRegistry::with_builtins();
    let document = SourceDocument::new("file:///motor.ld", 0, COMPLEX_LD);

    let out = registry.convert("ld", "st", &document);
    assert!(out.error.is_none(), "conversion error: {:?}", out.error);

    let st = &out.text;
    // Should have OR for the seal-in.
    assert!(st.contains("OR"), "ST was:\n{st}");
    // Should have AND and NOT.
    assert!(st.contains("AND"), "ST was:\n{st}");
    assert!(st.contains("NOT"), "ST was:\n{st}");
    // Should have the TON call.
    assert!(st.contains("Timer("), "ST was:\n{st}");
    // Should have MotorRun assignment.
    assert!(st.contains("MotorRun :="), "ST was:\n{st}");
}
