//! PLCopen XML (IEC 61131-10, v2.01-flavored) interchange for Ladder
//! Diagram models (PLC-115).
//!
//! Model-level conversion — deliberately NOT routed through the `plc_lang`
//! HIR hub: PLCopen interchange carries the *graphical* model (ids,
//! comments, rung structure), which the canonical HIR does not represent.
//! Implementations are written independently against the documented schema
//! (no vendor code — docs/licensing.md).
//!
//! - [`to_plcopen`] serializes an [`LdProgram`] to XML.
//! - [`from_plcopen`] parses the LD subset back, skipping unknown elements
//!   with fidelity notes.
//!
//! Positions are derived from the same grid math the webview uses — they
//! are cosmetic for other tools; our import ignores them and keeps the
//! `localId` as the element id.

mod export;
mod import;

pub use export::to_plcopen;
pub use import::{from_plcopen, from_plcopen_with_notes};

/// Crate-local error type for structured failures.
#[derive(Debug)]
pub struct PlcopenError(pub String);

impl std::fmt::Display for PlcopenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "plcopen: {}", self.0)
    }
}

impl std::error::Error for PlcopenError {}
