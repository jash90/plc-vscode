//! Ladder Diagram (LD) frontend — the graphical-language adapter.
//!
//! LD source is JSON (a serialized [`plc_ld::LdProgram`]).  `lower` parses the
//! JSON and delegates to [`plc_ld::lower_ld_program`] to build the canonical IR.
//! `can_render` is `false` because LD is graphical — conversion LD→ST goes
//! through the IR hub (LD lowers, ST renders), not LD rendering directly.

use plc_api::SourceDocument;
use plc_hir::HirModule;

use crate::{LanguageFrontend, LoweringResult};

/// Ladder Diagram (IEC 61131-3) language frontend.
pub struct LdFrontend;

/// Map an [`plc_ld::LdDiagnostic`] onto the API diagnostic shape.
///
/// LD is JSON, so there is no meaningful line/character span; the message
/// carries the rung index and element id instead.
fn to_api_diagnostic(d: plc_ld::LdDiagnostic) -> plc_api::Diagnostic {
    let location = match &d.element_id {
        Some(id) => format!("rung {}, element {id}: ", d.rung),
        None => format!("rung {}: ", d.rung),
    };
    plc_api::Diagnostic {
        severity: match d.severity {
            plc_ld::LdSeverity::Error => plc_api::DiagnosticSeverity::Error,
            plc_ld::LdSeverity::Warning => plc_api::DiagnosticSeverity::Warning,
        },
        range: plc_api::Range::at_start(),
        code: d.code,
        message: format!("{location}{}", d.message),
    }
}

impl LanguageFrontend for LdFrontend {
    fn id(&self) -> &'static str {
        "ld"
    }

    fn display_name(&self) -> &'static str {
        "Ladder Diagram"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ld"]
    }

    /// LD is graphical — it does not render the IR back to LD source.
    fn can_render(&self) -> bool {
        false
    }

    fn lower(&self, document: &SourceDocument) -> LoweringResult {
        let text = document.text();
        match plc_ld::parse_ld_json(text) {
            Ok(program) => {
                let module = plc_ld::lower_ld_program(&program);
                // Attach validation diagnostics. Only errors flow into
                // `LoweringResult` — the registry fails the conversion when
                // diagnostics are non-empty (SourceHasErrors), and warnings
                // must not block LD→ST round trips.
                let diagnostics = plc_ld::validate(&program)
                    .into_iter()
                    .filter(|d| d.severity == plc_ld::LdSeverity::Error)
                    .map(to_api_diagnostic)
                    .collect();
                LoweringResult {
                    module,
                    diagnostics,
                    fidelity: Vec::new(),
                }
            }
            Err(error) => {
                // Return an empty module with a diagnostic so conversion fails
                // gracefully (SourceHasErrors path in the registry).
                let diagnostic = plc_api::Diagnostic {
                    severity: plc_api::DiagnosticSeverity::Error,
                    range: plc_api::Range::at_start(),
                    code: "ld-parse",
                    message: format!("Invalid LD JSON: {error}"),
                };
                LoweringResult {
                    module: HirModule::default(),
                    diagnostics: vec![diagnostic],
                    fidelity: Vec::new(),
                }
            }
        }
    }
}
