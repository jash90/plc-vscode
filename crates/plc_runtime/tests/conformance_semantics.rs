//! Conformance suite — semantics slice.
//!
//! Representative Structured Text programs with the diagnostic codes the
//! compiler core is expected to surface. Extend `FIXTURES` as semantic coverage
//! grows.

use plc_compiler_core::{CompilerCore, SourceDocument};

struct SemanticFixture {
    name: &'static str,
    source: &'static str,
    expected_codes: &'static [&'static str],
}

const FIXTURES: &[SemanticFixture] = &[
    SemanticFixture {
        name: "clean_program",
        source: "PROGRAM Main\nVAR\n    Enabled : BOOL;\nEND_VAR\nEnabled := TRUE;\nEND_PROGRAM\n",
        expected_codes: &[],
    },
    SemanticFixture {
        name: "unresolved_target",
        source: "PROGRAM Main\nMissing := TRUE;\nEND_PROGRAM\n",
        expected_codes: &["SEM0001"],
    },
    SemanticFixture {
        name: "type_mismatch",
        source: "PROGRAM Main\nVAR\n    Enabled : BOOL;\nEND_VAR\nEnabled := 'yes';\nEND_PROGRAM\n",
        expected_codes: &["SEM0002"],
    },
];

#[test]
fn semantics_conformance_fixtures() {
    let core = CompilerCore::default();
    for fixture in FIXTURES {
        let document = SourceDocument::new(fixture.name, 1, fixture.source);
        let codes: Vec<&str> = core
            .analyze(&document)
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect();
        assert_eq!(
            codes, fixture.expected_codes,
            "semantic fixture `{}` diagnostic codes mismatch",
            fixture.name
        );
    }
}
