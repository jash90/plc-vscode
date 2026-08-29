//! PLC-115 — PLCopen XML (IEC 61131-10 flavored) interchange for LD:
//! export/import round-trips, differential lowering, byte-stable export,
//! and hostile-input handling. Independently implemented against the
//! documented v2.01 schema — no vendor code copied (docs/licensing.md).

use plc_ld::lower_ld_program;
use plc_ld::{LdProgram, parse_ld_json};
use plc_plcopen::{from_plcopen, from_plcopen_with_notes, to_plcopen};

/// Canonical v2 fixture (ids + comments), pinned by plc_ld tests.
const MOTOR_V2: &str = include_str!("../../../tests/ld/motor_control_v2.ld");

fn motor() -> LdProgram {
    parse_ld_json(MOTOR_V2).expect("v2 fixture parses")
}

#[test]
fn export_import_is_identity_on_motor_fixture() {
    let program = motor();
    let xml = to_plcopen(&program).expect("export");
    let back = from_plcopen(&xml).expect("import");
    assert_eq!(program, back, "deep equality incl. ids and comments");
}

#[test]
fn imported_xml_lowers_to_hir_identically() {
    // Differential: import → lower must equal direct lower.
    let program = motor();
    let xml = to_plcopen(&program).expect("export");
    let back = from_plcopen(&xml).expect("import");
    assert_eq!(lower_ld_program(&back), lower_ld_program(&program));
}

#[test]
fn exported_xml_is_stable_byte_for_byte() {
    let xml1 = to_plcopen(&motor()).expect("export 1");
    let xml2 = to_plcopen(&motor()).expect("export 2");
    assert_eq!(xml1, xml2);
}

#[test]
fn exported_xml_has_plcopen_shape() {
    let xml = to_plcopen(&motor()).expect("export");
    for needle in [
        "<project",
        "<pou",
        "pouType=\"program\"",
        "<LD>",
        "<leftPowerRail",
        "<contact",
        "<coil",
        "storage=\"reset\"",
        "<block",
        "<comment",
    ] {
        assert!(xml.contains(needle), "missing {needle} in:\n{xml}");
    }
}

#[test]
fn negated_contacts_and_set_reset_coils_roundtrip() {
    let mut program = LdProgram::new("Variants");
    program.rungs.push(plc_ld::Rung {
        id: Some("r0".to_owned()),
        comment: None,
        branches: vec![plc_ld::SeriesBranch {
            elements: vec![
                plc_ld::ContactElement {
                    id: Some("e0".to_owned()),
                    name: "A".to_owned(),
                    negated: false,
                },
                plc_ld::ContactElement {
                    id: Some("e1".to_owned()),
                    name: "B".to_owned(),
                    negated: true,
                },
            ],
        }],
        outputs: vec![
            plc_ld::OutputElement::Coil {
                id: Some("e2".to_owned()),
                name: "Latched".to_owned(),
                variant: plc_ld::CoilVariant::Set,
            },
            plc_ld::OutputElement::Coil {
                id: Some("e3".to_owned()),
                name: "Latched".to_owned(),
                variant: plc_ld::CoilVariant::Reset,
            },
        ],
    });
    let xml = to_plcopen(&program).expect("export");
    let back = from_plcopen(&xml).expect("import");
    assert_eq!(program, back);
}

#[test]
fn parallel_branches_roundtrip() {
    let mut program = LdProgram::new("Parallel");
    program.rungs.push(plc_ld::Rung {
        id: Some("r0".to_owned()),
        comment: Some("OR".to_owned()),
        branches: vec![
            plc_ld::SeriesBranch {
                elements: vec![plc_ld::ContactElement {
                    id: Some("e0".to_owned()),
                    name: "A".to_owned(),
                    negated: false,
                }],
            },
            plc_ld::SeriesBranch {
                elements: vec![
                    plc_ld::ContactElement {
                        id: Some("e1".to_owned()),
                        name: "B".to_owned(),
                        negated: false,
                    },
                    plc_ld::ContactElement {
                        id: Some("e2".to_owned()),
                        name: "C".to_owned(),
                        negated: true,
                    },
                ],
            },
        ],
        outputs: vec![plc_ld::OutputElement::Coil {
            id: Some("e3".to_owned()),
            name: "Out".to_owned(),
            variant: plc_ld::CoilVariant::Normal,
        }],
    });
    let xml = to_plcopen(&program).expect("export");
    let back = from_plcopen(&xml).expect("import");
    assert_eq!(program, back);
}

#[test]
fn block_pins_roundtrip() {
    let mut program = LdProgram::new("Timer");
    program.rungs.push(plc_ld::Rung {
        id: Some("r0".to_owned()),
        comment: None,
        branches: vec![plc_ld::SeriesBranch {
            elements: vec![plc_ld::ContactElement {
                id: Some("e0".to_owned()),
                name: "Start".to_owned(),
                negated: false,
            }],
        }],
        outputs: vec![plc_ld::OutputElement::Block {
            id: Some("e1".to_owned()),
            fb_type: "TON".to_owned(),
            instance: "Delay".to_owned(),
            inputs: vec![
                plc_ld::BlockArg {
                    name: "IN".to_owned(),
                    value: "Start".to_owned(),
                },
                plc_ld::BlockArg {
                    name: "PT".to_owned(),
                    value: "T#2s".to_owned(),
                },
            ],
            outputs: vec![plc_ld::BlockArg {
                name: "Q".to_owned(),
                value: "Done".to_owned(),
            }],
        }],
    });
    let xml = to_plcopen(&program).expect("export");
    let back = from_plcopen(&xml).expect("import");
    assert_eq!(program, back);
}

#[test]
fn malformed_xml_returns_structured_error() {
    let error = from_plcopen("<not-xml").expect_err("malformed XML must fail");
    let message = error.to_string();
    assert!(
        message.contains("plcopen") || message.contains("XML"),
        "{message}"
    );
}

#[test]
fn unknown_elements_are_skipped_with_fidelity_note() {
    // An FBD-era element we do not model must not fail the import.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0201">
  <types><dataTypes/><pous>
    <pou name="P" pouType="program">
      <body><LD>
        <leftPowerRail localId="1"/>
        <contact localId="2" negated="false"><position x="0" y="0"/><connectionPointIn><relPosition x="0" y="0"/><connection refLocalId="1"/></connectionPointIn><variable>A</variable></contact>
        <coil localId="3" storage="none"><position x="100" y="0"/><connectionPointIn><connection refLocalId="1"/></connectionPointIn><variable>Out</variable></coil>
        <inVariable localId="99"><position x="0" y="50"/><expression>Ghost</expression></inVariable>
      </LD></body>
    </pou>
  </pous></types>
</project>"#;
    let (program, notes) = from_plcopen_with_notes(xml).expect("import");
    assert_eq!(program.rungs.len(), 1);
    assert_eq!(program.rungs[0].branches[0].elements[0].name, "A");
    assert!(
        notes.iter().any(|note| note.contains("inVariable")),
        "unknown element noted: {notes:?}"
    );
}

#[test]
fn ids_survive_via_localid() {
    let program = motor();
    let xml = to_plcopen(&program).expect("export");
    assert!(
        xml.contains("localId=\"e0\""),
        "localId carries the element id"
    );
    let back = from_plcopen(&xml).expect("import");
    assert_eq!(back.rungs[0].id.as_deref(), Some("r0"));
}
