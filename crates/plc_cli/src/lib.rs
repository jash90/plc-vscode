//! Reusable CLI driver for PLC VS Code.
//!
//! The [`run_with`] driver is parameterized over the `plc_api` ports, so a
//! third-party binary can drive the same analyze-then-run flow with its own
//! [`LanguageService`] analyzer and/or its own [`ExecutionEngine`] backend. The
//! `plc` binary wires the default `CompilerCore` + `ScanRuntimeEngine`.

use plc_api::{ExecutionEngine, LanguageService, SourceDocument};
use plc_lang::LanguageRegistry;

/// Default scan-cycle interval, matching a typical cyclic task.
pub const SCAN_INTERVAL_MS: i64 = 100;
/// Default number of scan cycles so stateful function blocks (timers, counters)
/// reach an observable steady state.
pub const DEFAULT_SCANS: u64 = 25;

/// Analyze a document (diagnostics gate), then execute it through an engine and
/// print the resulting online "watch" snapshot.
///
/// Backend-agnostic: pass any `LanguageService` and any `ExecutionEngine`.
/// Errors (returned as `Err`) are the diagnostics-gate / load failures, with the
/// offending `code: message` lines already printed to stderr.
pub fn run_with(
    service: &dyn LanguageService,
    engine: &mut dyn ExecutionEngine,
    document: &SourceDocument,
    scans: u64,
) -> Result<(), String> {
    // Diagnostics gate: syntax/semantic errors block execution.
    let analysis = service.analyze(document);
    if !analysis.diagnostics().is_empty() {
        for diagnostic in analysis.diagnostics() {
            eprintln!("{}: {}", diagnostic.code, diagnostic.message);
        }
        return Err("execution failed due to diagnostics".to_owned());
    }

    // Compile/load through the engine; a real backend can surface its own
    // build diagnostics here.
    if let Err(diagnostics) = engine.load(document) {
        for diagnostic in &diagnostics {
            eprintln!("{}: {}", diagnostic.code, diagnostic.message);
        }
        return Err("execution failed due to diagnostics".to_owned());
    }

    engine.set_scan_interval_ms(SCAN_INTERVAL_MS);
    engine.run_scans(scans);

    let watch = engine.watch();
    if watch.is_empty() {
        println!("(no output)");
    } else {
        for line in watch {
            println!("{line}");
        }
    }
    Ok(())
}

/// Language-aware variant: pick the analyzer for `document` from `registry`
/// (by file extension), then run it through `engine` via [`run_with`]. Errors if
/// the document's language has no executable analyzer/service.
pub fn run_with_registry(
    registry: &LanguageRegistry,
    engine: &mut dyn ExecutionEngine,
    document: &SourceDocument,
    scans: u64,
) -> Result<(), String> {
    let service = registry
        .frontend_for_uri(document.uri())
        .and_then(|frontend| frontend.language_service())
        .ok_or_else(|| format!("no language service registered for `{}`", document.uri()))?;
    run_with(service.as_ref(), engine, document, scans)
}
