//! PLC-108 — LD structural validation: `LD00xx` diagnostic codes keyed by
//! stable element id, surfaced through `LdFrontend::lower()` so `plc convert`
//! fails loudly and the LSP Problems panel reports them.

use plc_ld::{
    BlockArg, CoilVariant, ContactElement, LdProgram, LdSeverity, OutputElement, Rung, SeriesBranch,
};

const MOTOR_LD: &str = include_str!("../../../tests/ld/motor_control.ld");
const MOTOR_LD_V2: &str = include_str!("../../../tests/ld/motor_control_v2.ld");

fn rung(contacts: &[&str], outputs: Vec<OutputElement>) -> Rung {
    Rung {
        id: None,
        comment: None,
        branches: vec![SeriesBranch {
            elements: contacts
                .iter()
                .map(|name| ContactElement {
                    id: None,
                    name: (*name).to_owned(),
                    negated: false,
                })
                .collect(),
        }],
        outputs,
    }
}

fn coil(name: &str) -> OutputElement {
    OutputElement::Coil {
        id: None,
        name: name.to_owned(),
        variant: CoilVariant::Normal,
    }
}

fn ton(instance: &str) -> OutputElement {
    OutputElement::Block {
        id: None,
        fb_type: "TON".to_owned(),
        instance: instance.to_owned(),
        inputs: vec![
            BlockArg {
                name: "IN".to_owned(),
                value: "A".to_owned(),
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
    }
}

fn codes(program: &LdProgram) -> Vec<(usize, &'static str)> {
    plc_ld::validate(program)
        .into_iter()
        .map(|d| (d.rung, d.code))
        .collect()
}

// ---------------------------------------------------------------------------
// LD0001 — empty rung / empty branch
// ---------------------------------------------------------------------------

#[test]
fn empty_rung_is_ld0001() {
    let mut program = LdProgram::new("P");
    program.rungs.push(Rung {
        id: None,
        comment: None,
        branches: vec![],
        outputs: vec![coil("Out")],
    });
    assert_eq!(codes(&program), vec![(0, "LD0001")]);
    assert_eq!(plc_ld::validate(&program)[0].severity, LdSeverity::Error);
}

#[test]
fn empty_branch_is_ld0001() {
    // A branch with no contacts lowers to Bool(true) — a silent always-on
    // rung. Flag it with the same code.
    let mut program = LdProgram::new("P");
    program.rungs.push(Rung {
        id: None,
        comment: None,
        branches: vec![SeriesBranch { elements: vec![] }],
        outputs: vec![coil("Out")],
    });
    assert_eq!(codes(&program), vec![(0, "LD0001")]);
}

// ---------------------------------------------------------------------------
// LD0002 — duplicate FB instance
// ---------------------------------------------------------------------------

#[test]
fn duplicate_fb_instance_is_ld0002() {
    let mut program = LdProgram::new("P");
    program.rungs.push(rung(&["A"], vec![ton("Timer")]));
    program.rungs.push(rung(&["B"], vec![ton("Timer")]));
    program.rungs[1].outputs[0] = match &program.rungs[1].outputs[0] {
        OutputElement::Block { id, .. } => OutputElement::Block {
            id: id.clone(),
            fb_type: "TON".to_owned(),
            instance: "Timer".to_owned(),
            inputs: vec![],
            outputs: vec![],
        },
        other => unreachable!("{other:?}"),
    };
    let diags = plc_ld::validate(&program);
    assert_eq!(
        diags.iter().map(|d| (d.rung, d.code)).collect::<Vec<_>>(),
        vec![(1, "LD0002")],
        "the second occurrence is flagged"
    );
    assert_eq!(diags[0].severity, LdSeverity::Error);
    assert!(
        diags[0].message.contains("Timer"),
        "message names the instance: {}",
        diags[0].message
    );
}

// ---------------------------------------------------------------------------
// LD0003 — unknown FB type
// ---------------------------------------------------------------------------

#[test]
fn unknown_fb_type_is_ld0003() {
    let mut program = LdProgram::new("P");
    program.rungs.push(rung(
        &["A"],
        vec![OutputElement::Block {
            id: None,
            fb_type: "MAGIC_TIMER".to_owned(),
            instance: "T1".to_owned(),
            inputs: vec![],
            outputs: vec![],
        }],
    ));
    let diags = plc_ld::validate(&program);
    assert_eq!(codes(&program), vec![(0, "LD0003")]);
    assert_eq!(diags[0].severity, LdSeverity::Error);
}

// ---------------------------------------------------------------------------
// LD0004 — missing outputs (warning)
// ---------------------------------------------------------------------------

#[test]
fn missing_outputs_is_ld0004() {
    let mut program = LdProgram::new("P");
    program.rungs.push(Rung {
        id: None,
        comment: None,
        branches: vec![SeriesBranch {
            elements: vec![ContactElement {
                id: None,
                name: "A".to_owned(),
                negated: false,
            }],
        }],
        outputs: vec![],
    });
    let diags = plc_ld::validate(&program);
    assert_eq!(codes(&program), vec![(0, "LD0004")]);
    assert_eq!(diags[0].severity, LdSeverity::Warning);
}

// ---------------------------------------------------------------------------
// LD0005 — unknown pin for a known FB (warning)
// ---------------------------------------------------------------------------

#[test]
fn unknown_pin_for_ton_is_ld0005() {
    let mut program = LdProgram::new("P");
    program.rungs.push(rung(
        &["A"],
        vec![OutputElement::Block {
            id: None,
            fb_type: "TON".to_owned(),
            instance: "T1".to_owned(),
            inputs: vec![BlockArg {
                name: "PRESET".to_owned(), // not a TON pin
                value: "T#2s".to_owned(),
            }],
            outputs: vec![],
        }],
    ));
    let diags = plc_ld::validate(&program);
    assert_eq!(codes(&program), vec![(0, "LD0005")]);
    assert_eq!(diags[0].severity, LdSeverity::Warning);
    assert!(
        diags[0].message.contains("PRESET"),
        "message names the pin: {}",
        diags[0].message
    );
}

// ---------------------------------------------------------------------------
// LD0006 — empty variable name
// ---------------------------------------------------------------------------

#[test]
fn empty_variable_name_is_ld0006() {
    let mut program = LdProgram::new("P");
    program.rungs.push(rung(&[""], vec![coil("Out")]));
    assert_eq!(codes(&program), vec![(0, "LD0006")]);
    assert_eq!(plc_ld::validate(&program)[0].severity, LdSeverity::Error);
}

// ---------------------------------------------------------------------------
// Clean fixtures
// ---------------------------------------------------------------------------

#[test]
fn valid_fixture_is_clean() {
    for (label, text) in [("v1", MOTOR_LD), ("v2", MOTOR_LD_V2)] {
        let program = plc_ld::parse_ld_json(text).unwrap();
        let diags = plc_ld::validate(&program);
        assert!(
            diags.is_empty(),
            "{label} fixture should be clean: {diags:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Element-id keying
// ---------------------------------------------------------------------------

#[test]
fn diagnostics_key_on_element_ids_when_present() {
    let mut program = plc_ld::parse_ld_json(MOTOR_LD_V2).unwrap();
    // Blank the contact name of rung 0's first contact (id e0).
    program.rungs[0].branches[0].elements[0].name = String::new();
    let diags = plc_ld::validate(&program);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code, "LD0006");
    assert_eq!(diags[0].element_id.as_deref(), Some("e0"));
}

// ---------------------------------------------------------------------------
// Review hardening (PLC-108 code review)
// ---------------------------------------------------------------------------

#[test]
fn lowercase_fb_type_is_accepted_like_the_runtime() {
    let mut program = LdProgram::new("P");
    program.rungs.push(rung(
        &["A"],
        vec![OutputElement::Block {
            id: None,
            fb_type: "ton".to_owned(),
            instance: "T1".to_owned(),
            inputs: vec![BlockArg {
                name: "in".to_owned(), // case-insensitive pin
                value: "A".to_owned(),
            }],
            outputs: vec![],
        }],
    ));
    assert!(
        plc_ld::validate(&program).is_empty(),
        "runtime treats types/pins case-insensitively: {:?}",
        plc_ld::validate(&program)
    );
}

#[test]
fn empty_instance_is_ld0006_not_ld0002() {
    let mut program = LdProgram::new("P");
    program.rungs.push(rung(
        &["A"],
        vec![OutputElement::Block {
            id: None,
            fb_type: "TON".to_owned(),
            instance: String::new(),
            inputs: vec![],
            outputs: vec![],
        }],
    ));
    assert_eq!(codes(&program), vec![(0, "LD0006")]);
}

#[test]
fn same_rung_duplicate_instance_is_ld0002() {
    let mut program = LdProgram::new("P");
    let mut rung = rung(&["A"], vec![ton("Timer")]);
    rung.outputs.push(ton("Timer"));
    program.rungs.push(rung);
    assert_eq!(codes(&program), vec![(0, "LD0002")]);
}

#[test]
fn instance_variable_collision_is_ld0007() {
    // A contact named like the FB instance would render duplicate VAR
    // declarations in ST (BOOL + the instance type).
    let mut program = LdProgram::new("P");
    program.rungs.push(rung(&["Timer"], vec![ton("Timer")]));
    assert_eq!(codes(&program), vec![(0, "LD0007")]);
}

#[test]
fn empty_coil_name_is_ld0006() {
    let mut program = LdProgram::new("P");
    program.rungs.push(rung(&["A"], vec![coil("")]));
    assert_eq!(codes(&program), vec![(0, "LD0006")]);
}

#[test]
fn unknown_fb_type_suppresses_ld0005() {
    // No pin table exists for an unknown type; pin checking is skipped.
    let mut program = LdProgram::new("P");
    program.rungs.push(rung(
        &["A"],
        vec![OutputElement::Block {
            id: None,
            fb_type: "MAGIC".to_owned(),
            instance: "T1".to_owned(),
            inputs: vec![BlockArg {
                name: "WHATEVER".to_owned(),
                value: "A".to_owned(),
            }],
            outputs: vec![],
        }],
    ));
    assert_eq!(codes(&program), vec![(0, "LD0003")]);
}

#[test]
fn literal_output_pin_value_is_ld0006() {
    // Output-pin values become assignment targets; a literal renders an ST
    // statement the runtime silently skips.
    let mut program = LdProgram::new("P");
    program.rungs.push(rung(
        &["A"],
        vec![OutputElement::Block {
            id: None,
            fb_type: "TON".to_owned(),
            instance: "T1".to_owned(),
            inputs: vec![BlockArg {
                name: "IN".to_owned(),
                value: "A".to_owned(),
            }],
            outputs: vec![BlockArg {
                name: "Q".to_owned(),
                value: "5".to_owned(), // literal, not a variable
            }],
        }],
    ));
    assert_eq!(codes(&program), vec![(0, "LD0006")]);
}
