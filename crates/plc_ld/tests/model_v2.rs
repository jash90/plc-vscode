//! PLC-107 — LD model v2: schema versioning, element ids, rung comments, and
//! per-contact power flow.
//!
//! These tests pin the additive v2 wire format: `schema_version` on the
//! program, optional `id` on rungs/contacts/outputs, optional `comment` on
//! rungs, and cumulative per-contact energization in power-flow results.

use plc_ld::{
    CoilVariant, ContactElement, LdProgram, OutputElement, Rung, SeriesBranch, evaluate_power_flow,
    normalize_ids, parse_ld_json,
};

const MOTOR_LD: &str = include_str!("../../../tests/ld/motor_control.ld");
const MOTOR_LD_V2: &str = include_str!("../../../tests/ld/motor_control_v2.ld");

#[test]
fn v2_fixture_is_already_normalized() {
    let mut program = parse_ld_json(MOTOR_LD_V2).expect("v2 fixture parses");
    let normalized = program.clone();
    normalize_ids(&mut program);
    assert_eq!(program, normalized, "v2 fixture ids are canonical");
}

#[test]
fn v2_fixture_round_trips() {
    let program = parse_ld_json(MOTOR_LD_V2).unwrap();
    let back = parse_ld_json(&serde_json::to_string_pretty(&program).unwrap()).unwrap();
    assert_eq!(program, back);
}

fn no_id_rung(contacts: &[(&str, bool)]) -> Rung {
    Rung {
        id: None,
        comment: None,
        branches: vec![SeriesBranch {
            elements: contacts
                .iter()
                .map(|(name, negated)| ContactElement {
                    id: None,
                    name: (*name).to_owned(),
                    negated: *negated,
                })
                .collect(),
        }],
        outputs: vec![OutputElement::Coil {
            id: None,
            name: "Out".to_owned(),
            variant: CoilVariant::Normal,
        }],
    }
}

// ---------------------------------------------------------------------------
// Comments + schema version (wire format v2)
// ---------------------------------------------------------------------------

#[test]
fn comment_survives_round_trip() {
    let program = parse_ld_json(MOTOR_LD).expect("fixture parses");
    assert!(
        program.rungs.iter().all(|r| r.comment.is_some()),
        "fixture carries a comment on every rung"
    );

    let json = serde_json::to_string_pretty(&program).unwrap();
    let back = parse_ld_json(&json).unwrap();
    assert_eq!(program, back);
    assert_eq!(
        back.rungs[0].comment.as_deref(),
        Some("Seal-in motor: (Start OR Motor) AND NOT Stop -> MotorRun")
    );
}

#[test]
fn legacy_fixture_parses_without_schema_version() {
    // The v1 fixture has no `schema_version` field; it must load and read as
    // the current (additive-superset) version.
    let program = parse_ld_json(MOTOR_LD).expect("legacy fixture parses");
    assert_eq!(
        program.schema_version,
        plc_ld::model::CURRENT_SCHEMA_VERSION
    );
}

#[test]
fn round_trip_emits_schema_version() {
    let program = parse_ld_json(MOTOR_LD).unwrap();
    let json = serde_json::to_string(&program).unwrap();
    assert!(
        json.contains("\"schema_version\""),
        "serialized JSON: {json}"
    );
}

// ---------------------------------------------------------------------------
// Element ids
// ---------------------------------------------------------------------------

#[test]
fn normalize_assigns_stable_ids() {
    let mut program = parse_ld_json(MOTOR_LD).unwrap();
    normalize_ids(&mut program);

    assert_eq!(program.rungs[0].id.as_deref(), Some("r0"));
    assert_eq!(program.rungs[1].id.as_deref(), Some("r1"));
    // Rung 0: branch 0 contact Start, branch 1 contact Motor, coil MotorRun.
    assert_eq!(
        program.rungs[0].branches[0].elements[0].id.as_deref(),
        Some("e0")
    );
    assert_eq!(
        program.rungs[0].branches[1].elements[0].id.as_deref(),
        Some("e1")
    );
    match &program.rungs[0].outputs[0] {
        OutputElement::Coil { id, .. } => assert_eq!(id.as_deref(), Some("e2")),
        other => panic!("expected coil, got {other:?}"),
    }

    // Deterministic: normalizing an already-normalized program changes nothing.
    let once = program.clone();
    normalize_ids(&mut program);
    assert_eq!(once, program);
}

#[test]
fn normalize_preserves_existing_ids() {
    let mut program = LdProgram::new("P");
    program.rungs.push(no_id_rung(&[("A", false)]));
    program.rungs[0].id = Some("r-custom".to_owned());
    program.rungs[0].branches[0].elements[0].id = Some("e-keep".to_owned());

    normalize_ids(&mut program);

    assert_eq!(program.rungs[0].id.as_deref(), Some("r-custom"));
    assert_eq!(
        program.rungs[0].branches[0].elements[0].id.as_deref(),
        Some("e-keep")
    );
}

#[test]
fn normalize_resolves_collisions() {
    let mut program = LdProgram::new("P");
    program
        .rungs
        .push(no_id_rung(&[("A", false), ("B", false)]));
    // Both contacts claim the same id.
    program.rungs[0].branches[0].elements[0].id = Some("e0".to_owned());
    program.rungs[0].branches[0].elements[1].id = Some("e0".to_owned());

    normalize_ids(&mut program);

    let ids: Vec<&str> = program.rungs[0].branches[0]
        .elements
        .iter()
        .map(|c| c.id.as_deref().unwrap())
        .collect();
    assert_eq!(ids.len(), 2);
    let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
    assert_eq!(
        unique.len(),
        2,
        "ids must be unique after normalize: {ids:?}"
    );
    // The first occurrence keeps the claimed id; the second is regenerated.
    assert_eq!(ids[0], "e0");
}

#[test]
fn next_id_skips_used() {
    let mut program = LdProgram::new("P");
    program
        .rungs
        .push(no_id_rung(&[("A", false), ("B", false)]));
    program.rungs[0].branches[0].elements[0].id = Some("e7".to_owned());

    normalize_ids(&mut program);

    // e7 is preserved; the id-less contact must not reuse 7.
    assert_eq!(
        program.rungs[0].branches[0].elements[0].id.as_deref(),
        Some("e7")
    );
    assert_ne!(
        program.rungs[0].branches[0].elements[1].id.as_deref(),
        Some("e7")
    );
}

// ---------------------------------------------------------------------------
// Per-contact power flow
// ---------------------------------------------------------------------------

#[test]
fn contact_energized_reports_cumulative_and_state() {
    let mut program = LdProgram::new("P");
    program.rungs.push(no_id_rung(&[("A", false), ("B", true)]));

    let mut state = std::collections::HashMap::new();
    state.insert("a".to_owned(), true);
    state.insert("b".to_owned(), false);

    let result = evaluate_power_flow(&program, &state);
    // A (true) passes; A AND NOT B (B=false → NC passes) stays true.
    assert_eq!(result.rungs[0].contact_energized, vec![vec![true, true]]);

    // Now B=true: contact 0 still energized, the chain after B goes dead.
    state.insert("b".to_owned(), true);
    let result = evaluate_power_flow(&program, &state);
    assert_eq!(result.rungs[0].contact_energized, vec![vec![true, false]]);
}
