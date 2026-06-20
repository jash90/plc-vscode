//! End-to-end golden test: load the bundled CPDev `WeJeStSt.xcp` program (with
//! its `.dcp` variable map), run scan cycles through `XcpEngine`, and assert the
//! watch snapshot. The fixtures are vendored from upstream (binary), so they are
//! embedded with `include_bytes!` and written to a temp dir as an `.xcp`/`.dcp`
//! sidecar pair (the engine resolves the `.dcp` next to the `.xcp`).

use plc_api::{ExecutionEngine, SourceDocument};
use plc_cpdev_vm::XcpEngine;

const XCP: &[u8] = include_bytes!("fixtures/WeJeStSt.xcp");
const DCP: &[u8] = include_bytes!("fixtures/WeJeStSt.dcp");

fn load_engine(dir: &std::path::Path) -> (XcpEngine, SourceDocument) {
    let xcp_path = dir.join("WeJeStSt.xcp");
    std::fs::write(&xcp_path, XCP).unwrap();
    std::fs::write(dir.join("WeJeStSt.dcp"), DCP).unwrap();
    let uri = format!("file://{}", xcp_path.display());
    let doc = SourceDocument::new(uri, 0, String::new());
    let mut engine = XcpEngine::default();
    engine.load(&doc).expect("load WeJeStSt.xcp + .dcp");
    (engine, doc)
}

/// The deterministic watch snapshot of `WeJeStSt` after 25 free-run scans:
/// the program drives OUT0 high and leaves the rest low.
fn expected() -> Vec<String> {
    vec![
        "OUT0 = TRUE".to_owned(),
        "OUT1 = FALSE".to_owned(),
        "OUT2 = FALSE".to_owned(),
        "OUT3 = FALSE".to_owned(),
        "ONOF = FALSE".to_owned(),
    ]
}

#[test]
fn runs_wejestst_and_watches_declared_globals() {
    let dir = tempfile::tempdir().unwrap();
    let (mut engine, _doc) = load_engine(dir.path());

    engine.set_scan_interval_ms(0);
    engine.run_scans(25);

    assert_eq!(engine.watch(), expected());
}

#[test]
fn load_artifact_from_bytes_matches_file_load() {
    // The bytes-based port entry (load_artifact) must reach the same state as
    // the file-based load; the .dcp sidecar is still resolved from the uri.
    let dir = tempfile::tempdir().unwrap();
    let xcp_path = dir.path().join("WeJeStSt.xcp");
    std::fs::write(&xcp_path, XCP).unwrap();
    std::fs::write(dir.path().join("WeJeStSt.dcp"), DCP).unwrap();
    let uri = format!("file://{}", xcp_path.display());

    let mut engine = XcpEngine::default();
    engine.load_artifact(XCP, &uri).expect("load_artifact");
    engine.run_scans(25);

    assert_eq!(engine.watch(), expected());
}

#[test]
fn set_input_writes_through_to_vm() {
    // Exercises the write path: set_input -> cpdev_set -> WM_SetData. Staging the
    // ONOF input and running must stay deterministic and not corrupt the others.
    let dir = tempfile::tempdir().unwrap();
    let (mut engine, _) = load_engine(dir.path());
    engine.set_input("ONOF", "TRUE");
    engine.run_scans(25);
    let watch = engine.watch();
    // ONOF reads back exactly what we wrote; the run stays well-formed.
    assert!(
        watch.contains(&"ONOF = TRUE".to_owned()),
        "watch: {watch:#?}"
    );
    assert_eq!(watch.len(), 5);
}

#[test]
fn run_is_deterministic_across_reloads() {
    let dir = tempfile::tempdir().unwrap();
    let (mut a, _) = load_engine(dir.path());
    a.run_scans(25);
    let (mut b, _) = load_engine(dir.path());
    b.run_scans(25);
    assert_eq!(a.watch(), b.watch());
}
