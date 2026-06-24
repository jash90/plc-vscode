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

pub mod lower;
pub mod model;
pub mod power_flow;

pub use lower::lower_ld_program;
pub use model::*;
pub use power_flow::{evaluate_power_flow, var_state_from_watch};

/// Parse an [`LdProgram`] from a JSON string.
pub fn parse_ld_json(text: &str) -> Result<LdProgram, serde_json::Error> {
    serde_json::from_str(text)
}
