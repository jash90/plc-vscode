//! Conformance suite — parser slice.
//!
//! Representative Structured Text programs with their expected parser
//! diagnostic counts. Extend `FIXTURES` as parser coverage grows.

use plc_syntax::parse_source;

struct ParserFixture {
    name: &'static str,
    source: &'static str,
    expected_diagnostics: usize,
}

const FIXTURES: &[ParserFixture] = &[
    ParserFixture {
        name: "minimal_program",
        source: "PROGRAM Main\nVAR\nEND_VAR\nEND_PROGRAM\n",
        expected_diagnostics: 0,
    },
    ParserFixture {
        name: "missing_terminator",
        source: "PROGRAM Main\nVAR\nEND_VAR\n",
        expected_diagnostics: 1,
    },
    ParserFixture {
        name: "recovers_following_pou",
        source: "PROGRAM Broken\nVAR\nEND_VAR\nFUNCTION_BLOCK Motor\nEND_FUNCTION_BLOCK\n",
        expected_diagnostics: 1,
    },
];

#[test]
fn parser_conformance_fixtures() {
    for fixture in FIXTURES {
        let parsed = parse_source(fixture.source);
        assert_eq!(
            parsed.diagnostics().len(),
            fixture.expected_diagnostics,
            "parser fixture `{}` diagnostic count mismatch",
            fixture.name
        );
    }
}
