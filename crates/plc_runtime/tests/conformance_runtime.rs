//! Conformance suite — runtime slice.
//!
//! Representative Structured Text programs executed through the deterministic
//! runtime with their expected post-execution state. Extend `FIXTURES` as
//! runtime coverage grows.

use plc_runtime::{Runtime, Value};

struct RuntimeFixture {
    name: &'static str,
    source: &'static str,
    scans: u64,
    expected: &'static [(&'static str, Value)],
}

fn fixtures() -> Vec<RuntimeFixture> {
    vec![
        RuntimeFixture {
            name: "counter_increments",
            source: "PROGRAM Main\nVAR\n    Count : INT;\nEND_VAR\nCount := Count + 1;\nEND_PROGRAM\n",
            scans: 4,
            expected: &[("Count", Value::Int(4))],
        },
        RuntimeFixture {
            name: "boolean_latch",
            source: "PROGRAM Main\nVAR\n    Lamp : BOOL;\nEND_VAR\nLamp := TRUE;\nEND_PROGRAM\n",
            scans: 1,
            expected: &[("Lamp", Value::Bool(true))],
        },
        RuntimeFixture {
            name: "real_accumulator",
            source: "PROGRAM Main\nVAR\n    Total : REAL;\nEND_VAR\nTotal := Total + 2;\nEND_PROGRAM\n",
            scans: 3,
            expected: &[("Total", Value::Real(6.0))],
        },
    ]
}

#[test]
fn runtime_conformance_fixtures() {
    for fixture in fixtures() {
        let mut runtime = Runtime::from_source(fixture.source);
        runtime.run_scans(fixture.scans);
        for (name, expected) in fixture.expected {
            assert_eq!(
                runtime.value(name),
                Some(expected),
                "runtime fixture `{}` variable `{name}` mismatch",
                fixture.name
            );
        }
    }
}
