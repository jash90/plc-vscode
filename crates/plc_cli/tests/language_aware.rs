//! Proves the CLI's `run_with_registry` selects the analyzer by file extension
//! (language-aware), and refuses to run a language with no executable backend.

use plc_api::{Diagnostic, ExecutionEngine, SourceDocument};
use plc_cli::run_with_registry;
use plc_lang::LanguageRegistry;

#[derive(Default)]
struct RecordingEngine {
    loaded: bool,
    scans: u64,
    snapshot: Vec<String>,
}

impl ExecutionEngine for RecordingEngine {
    fn load(&mut self, _document: &SourceDocument) -> Result<(), Vec<Diagnostic>> {
        self.loaded = true;
        self.snapshot = vec!["ran = 1".to_owned()];
        Ok(())
    }
    fn set_scan_interval_ms(&mut self, _scan_interval_ms: i64) {}
    fn run_scans(&mut self, cycles: u64) {
        self.scans = cycles;
    }
    fn set_input(&mut self, _name: &str, _value: &str) {}
    fn watch(&self) -> Vec<String> {
        self.snapshot.clone()
    }
}

#[test]
fn run_with_registry_dispatches_structured_text_by_extension() {
    let registry = LanguageRegistry::with_builtins();
    let document = SourceDocument::new(
        "file:///main.st",
        0,
        "PROGRAM Main\nVAR\n    x : INT;\nEND_VAR\nx := 1;\nEND_PROGRAM\n",
    );
    let mut engine = RecordingEngine::default();

    run_with_registry(&registry, &mut engine, &document, 3)
        .expect(".st dispatches to CompilerCore");
    assert!(engine.loaded);
    assert_eq!(engine.scans, 3);
}

#[test]
fn run_with_registry_refuses_a_non_executable_language() {
    let registry = LanguageRegistry::with_builtins();
    // IL has no executable language service -> running it is refused (not garbage).
    let document = SourceDocument::new("file:///main.il", 0, "PROGRAM Main\nEND_PROGRAM\n");
    let mut engine = RecordingEngine::default();

    assert!(run_with_registry(&registry, &mut engine, &document, 1).is_err());
    assert!(!engine.loaded);
}
