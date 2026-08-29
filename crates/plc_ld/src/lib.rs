//! Ladder Diagram (LD) module for PLC VS Code.
//!
//! This crate provides:
//!
//! - **Model** ([`model`]): a serde-serializable representation of LD programs
//!   (rungs, contacts, coils, function-block invocations) stored as `.ld` JSON.
//! - **Lowering** ([`lower`]): converts an [`LdProgram`] into a
//!   [`plc_hir::HirModule`] so LD can be rendered to ST and executed by the
//!   existing runtime via the IR hub.
//! - **Power-flow** ([`power_flow`]): evaluates which elements in a rung are
//!   energized given a variable state — used for live visualization.

pub mod ids;
pub mod lower;
pub mod model;
pub mod power_flow;
pub mod validate;

pub use ids::normalize_ids;
pub use lower::lower_ld_program;
pub use model::*;
pub use power_flow::{evaluate_power_flow, var_state_from_watch};
pub use validate::{
    LdDiagnostic, LdSeverity, STANDARD_FB_TYPES, fb_pins, is_standard_fb, validate,
};

/// Parse an [`LdProgram`] from a JSON string.
///
/// Files declaring a `schema_version` newer than [`CURRENT_SCHEMA_VERSION`]
/// are rejected loudly rather than silently misinterpreted.
pub fn parse_ld_json(text: &str) -> Result<LdProgram, serde_json::Error> {
    use serde::de::Error as _;

    let program: LdProgram = serde_json::from_str(text)?;
    if program.schema_version > model::CURRENT_SCHEMA_VERSION {
        return Err(serde_json::Error::custom(format!(
            "unsupported .ld schema_version {} (this build understands up to {})",
            program.schema_version,
            model::CURRENT_SCHEMA_VERSION
        )));
    }
    Ok(program)
}
