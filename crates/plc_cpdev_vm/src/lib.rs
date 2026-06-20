//! CPDev IEC 61131-3 bytecode (`.XCP`) virtual machine as a pluggable
//! [`plc_api::ExecutionEngine`].
//!
//! This crate vendors the upstream CPDev VM (C++; see `vendor/cpdev/UPSTREAM.md`)
//! and drives it through a thin `extern "C"` shim (`shim/`). [`XcpEngine`] maps
//! the engine port onto the VM's scan-cycle model:
//!
//! ```text
//! load        -> VMP_LoadProgramAndData(.xcp) + VMDCP::Load(.dcp sidecar)
//! set_input   -> WM_SetData(handle, le-bytes)   (staged, flushed each scan)
//! run_scans   -> WM_Initialize once, then WM_RunCycle * N
//! watch       -> WM_GetData(handle) per declared variable -> "name = value"
//! ```
//!
//! The `.DCP` is a small XML map of declared variables (name/size/type). We parse
//! it on the Rust side to enumerate variables in declared order (the VM resolves
//! one name at a time and does not list them), and read each variable's IEC type
//! from it for value formatting (the VM tracks only size, not type).

use std::ffi::{CString, c_void};
use std::os::raw::c_char;
use std::path::{Path, PathBuf};

use plc_api::{Diagnostic, DiagnosticSeverity, ExecutionEngine, Range, SourceDocument};

// ---------------------------------------------------------------------------
// FFI surface (see shim/cpdev_shim.h). Hand-written: the surface is tiny and
// has no structs/overloads to mirror, so bindgen would only add a build dep.
// ---------------------------------------------------------------------------

/// Opaque handle to the C++ `CpdevVm` owned by the shim.
#[repr(C)]
pub struct CpdevVm {
    _private: [u8; 0],
}

unsafe extern "C" {
    fn cpdev_open() -> *mut CpdevVm;
    fn cpdev_free(vm: *mut CpdevVm);
    fn cpdev_load_xcp_file(vm: *mut CpdevVm, path: *const c_char) -> i32;
    fn cpdev_load_xcp(vm: *mut CpdevVm, code: *const u8, len: i32) -> i32;
    fn cpdev_load_dcp(vm: *mut CpdevVm, path: *const c_char) -> i32;
    fn cpdev_var(vm: *mut CpdevVm, name: *const c_char) -> *mut c_void;
    fn cpdev_var_size(var: *mut c_void) -> i32;
    fn cpdev_set_task_cycle(vm: *mut CpdevVm, ms: u16);
    fn cpdev_initialize(vm: *mut CpdevVm, mode: i32);
    fn cpdev_run_cycle(vm: *mut CpdevVm);
    fn cpdev_set(vm: *mut CpdevVm, var: *mut c_void, buf: *const u8, len: i32) -> i32;
    fn cpdev_get(vm: *mut CpdevVm, var: *mut c_void, buf: *mut u8, len: i32) -> i32;
    fn cpdev_status1(vm: *mut CpdevVm) -> u16;
    fn cpdev_run_mode(vm: *mut CpdevVm) -> u8;
}

/// `WM_MODE_FIRST_START | WM_MODE_NORMAL` — the one-time initialization mode.
const INIT_MODE: i32 = 0x40 | 0x01;
/// `WMSTAT_BADFORMAT` — set by the VM when the loaded code is not valid XCP.
const WMSTAT_BADFORMAT: u16 = 0x0010;

// ---------------------------------------------------------------------------
// IEC value types: parse text -> little-endian bytes, decode bytes -> display.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IecType {
    Bool,
    Unsigned, // BYTE/WORD/DWORD/LWORD/USINT/UINT/UDINT/ULINT
    Signed,   // SINT/INT/DINT/LINT
    Real,
    Lreal,
    Time,
    Str,
    Unknown,
}

impl IecType {
    fn from_dcp(name: &str) -> Self {
        match name.to_ascii_uppercase().as_str() {
            "BOOL" => Self::Bool,
            "SINT" | "INT" | "DINT" | "LINT" => Self::Signed,
            "BYTE" | "WORD" | "DWORD" | "LWORD" | "USINT" | "UINT" | "UDINT" | "ULINT" => {
                Self::Unsigned
            }
            "REAL" => Self::Real,
            "LREAL" => Self::Lreal,
            "TIME" | "TOD" | "TIME_OF_DAY" => Self::Time,
            "STRING" | "WSTRING" => Self::Str,
            _ => Self::Unknown,
        }
    }

    /// Parse a textual input value into the variable's little-endian byte image.
    fn encode(self, text: &str, size: usize) -> Option<Vec<u8>> {
        let text = text.trim();
        match self {
            Self::Bool => {
                let on = matches!(text.to_ascii_uppercase().as_str(), "TRUE" | "1");
                let off = matches!(text.to_ascii_uppercase().as_str(), "FALSE" | "0");
                if !on && !off {
                    return None;
                }
                Some(vec![u8::from(on)])
            }
            Self::Signed | Self::Unsigned | Self::Time => {
                let value: i128 = text.parse().ok()?;
                let le = value.to_le_bytes();
                Some(le.get(..size).map(<[u8]>::to_vec).unwrap_or_else(|| {
                    let mut bytes = le.to_vec();
                    bytes.resize(size, if value < 0 { 0xFF } else { 0x00 });
                    bytes
                }))
            }
            Self::Real => Some(text.parse::<f32>().ok()?.to_le_bytes().to_vec()),
            Self::Lreal => Some(text.parse::<f64>().ok()?.to_le_bytes().to_vec()),
            Self::Str => {
                let mut bytes = text.as_bytes().to_vec();
                bytes.resize(size, 0);
                Some(bytes)
            }
            Self::Unknown => None,
        }
    }

    /// Decode a variable's little-endian byte image into its watch display.
    fn decode(self, bytes: &[u8]) -> String {
        match self {
            Self::Bool => {
                if bytes.iter().any(|&b| b != 0) {
                    "TRUE".to_owned()
                } else {
                    "FALSE".to_owned()
                }
            }
            Self::Unsigned => {
                let mut buf = [0u8; 16];
                buf[..bytes.len().min(16)].copy_from_slice(&bytes[..bytes.len().min(16)]);
                u128::from_le_bytes(buf).to_string()
            }
            Self::Signed => sign_extend(bytes).to_string(),
            Self::Real => {
                let mut buf = [0u8; 4];
                buf[..bytes.len().min(4)].copy_from_slice(&bytes[..bytes.len().min(4)]);
                real_to_string(f32::from_le_bytes(buf) as f64)
            }
            Self::Lreal => {
                let mut buf = [0u8; 8];
                buf[..bytes.len().min(8)].copy_from_slice(&bytes[..bytes.len().min(8)]);
                real_to_string(f64::from_le_bytes(buf))
            }
            Self::Time => {
                let mut buf = [0u8; 8];
                buf[..bytes.len().min(8)].copy_from_slice(&bytes[..bytes.len().min(8)]);
                time_to_string(u64::from_le_bytes(buf) as i64)
            }
            Self::Str => {
                let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
                String::from_utf8_lossy(&bytes[..end]).into_owned()
            }
            Self::Unknown => {
                // No type info: show raw little-endian hex so nothing is hidden.
                let mut out = String::from("0x");
                for byte in bytes.iter().rev() {
                    out.push_str(&format!("{byte:02X}"));
                }
                out
            }
        }
    }
}

/// Sign-extend a little-endian two's-complement byte slice to `i128`.
fn sign_extend(bytes: &[u8]) -> i128 {
    let mut buf = [0u8; 16];
    let len = bytes.len().min(16);
    buf[..len].copy_from_slice(&bytes[..len]);
    if len > 0 && len < 16 && bytes[len - 1] & 0x80 != 0 {
        for byte in &mut buf[len..] {
            *byte = 0xFF;
        }
    }
    i128::from_le_bytes(buf)
}

/// CODESYS-style REAL rendering (whole reals keep a trailing `.0`), matching
/// `plc_runtime::stdlib::real_to_string` so watch output is consistent.
fn real_to_string(value: f64) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    if value == value.trunc() && value.abs() < 1e15 {
        return format!("{value:.1}");
    }
    let rendered = format!("{value:.6}");
    let trimmed = rendered.trim_end_matches('0');
    if trimmed.ends_with('.') {
        format!("{trimmed}0")
    } else {
        trimmed.to_owned()
    }
}

/// IEC `T#` compound duration rendering, matching `plc_runtime`.
fn time_to_string(ms: i64) -> String {
    if ms == 0 {
        return "T#0ms".to_owned();
    }
    let negative = ms < 0;
    let mut remaining = ms.unsigned_abs();
    let mut out = String::from("T#");
    if negative {
        out.push('-');
    }
    for (unit_ms, suffix) in [
        (86_400_000u64, "d"),
        (3_600_000, "h"),
        (60_000, "m"),
        (1_000, "s"),
        (1, "ms"),
    ] {
        let amount = remaining / unit_ms;
        if amount > 0 {
            out.push_str(&amount.to_string());
            out.push_str(suffix);
            remaining %= unit_ms;
        }
    }
    out
}

// ---------------------------------------------------------------------------
// .DCP variable map: a tiny scanner for `<VAR LName=".." Size=".." Type=".." />`.
// ---------------------------------------------------------------------------

struct DcpVar {
    name: String,
    ty: IecType,
}

/// Extract declared variables (in document order) from the `.DCP` XML text.
/// Deliberately minimal — the format is one `<VAR .../>` element per variable.
fn parse_dcp_vars(xml: &str) -> Vec<DcpVar> {
    let mut vars = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<VAR ") {
        rest = &rest[start + 5..];
        let end = rest.find('>').unwrap_or(rest.len());
        let tag = &rest[..end];
        rest = &rest[end..];
        if let Some(name) = attr(tag, "LName") {
            let ty = attr(tag, "Type")
                .map(|t| IecType::from_dcp(&t))
                .unwrap_or(IecType::Unknown);
            vars.push(DcpVar { name, ty });
        }
    }
    vars
}

/// Read a `key="value"` attribute out of an XML start-tag's attribute text.
fn attr(tag: &str, key: &str) -> Option<String> {
    let mut search = tag;
    let needle_eq = format!("{key}=\"");
    while let Some(pos) = search.find(&needle_eq) {
        // Ensure the match is a whole attribute name (preceded by start or space).
        let preceded_ok = pos == 0 || search.as_bytes()[pos - 1] == b' ';
        let after = &search[pos + needle_eq.len()..];
        if preceded_ok {
            let end = after.find('"')?;
            return Some(after[..end].to_owned());
        }
        search = after;
    }
    None
}

// ---------------------------------------------------------------------------
// XcpEngine
// ---------------------------------------------------------------------------

struct VarDecl {
    name: String,
    handle: *mut c_void,
    size: usize,
    ty: IecType,
}

/// Pluggable execution engine backed by the vendored CPDev VM.
pub struct XcpEngine {
    vm: *mut CpdevVm,
    vars: Vec<VarDecl>,
    /// Pending input writes (variable index -> little-endian bytes), re-applied
    /// at the start of every scan cycle (mirrors writing inputs before a scan).
    staged: Vec<(usize, Vec<u8>)>,
    initialized: bool,
}

impl Default for XcpEngine {
    fn default() -> Self {
        Self {
            vm: std::ptr::null_mut(),
            vars: Vec::new(),
            staged: Vec::new(),
            initialized: false,
        }
    }
}

impl Drop for XcpEngine {
    fn drop(&mut self) {
        if !self.vm.is_null() {
            // SAFETY: `vm` was produced by cpdev_open and not yet freed; the
            // variable handles it owns are freed by cpdev_free, so we must not
            // touch `self.vars` afterwards (we are being dropped).
            unsafe { cpdev_free(self.vm) };
            self.vm = std::ptr::null_mut();
        }
    }
}

impl XcpEngine {
    /// Open a fresh VM, discarding any previously loaded program.
    fn reset(&mut self) -> Result<(), Vec<Diagnostic>> {
        if !self.vm.is_null() {
            // SAFETY: valid handle from a prior open; consumed here.
            unsafe { cpdev_free(self.vm) };
        }
        self.vars.clear();
        self.staged.clear();
        self.initialized = false;
        // SAFETY: no arguments; returns an owned handle or null.
        self.vm = unsafe { cpdev_open() };
        if self.vm.is_null() {
            return Err(err(
                "E_VM_OPEN",
                "failed to allocate the CPDev VM".to_owned(),
            ));
        }
        Ok(())
    }

    /// Load the `.DCP` sidecar next to `xcp_path`, then resolve every declared
    /// variable into a handle. Shared by `load` and `load_artifact`.
    fn load_dcp_and_vars(&mut self, xcp_path: &Path) -> Result<(), Vec<Diagnostic>> {
        let dcp_path = xcp_path.with_extension("dcp");
        let dcp_c = path_to_cstring(&dcp_path)?;
        // SAFETY: vm is non-null (reset ran); dcp_c is a valid NUL-terminated path.
        let rc = unsafe { cpdev_load_dcp(self.vm, dcp_c.as_ptr()) };
        if rc != 0 {
            return Err(err(
                "E_DCP_LOAD",
                format!(
                    "failed to load DCP variable map `{}` (code {rc})",
                    dcp_path.display()
                ),
            ));
        }

        let xml = std::fs::read_to_string(&dcp_path).map_err(|e| {
            err(
                "E_DCP_READ",
                format!("failed to read DCP `{}`: {e}", dcp_path.display()),
            )
        })?;
        for var in parse_dcp_vars(&xml) {
            let name_c = match CString::new(var.name.as_str()) {
                Ok(c) => c,
                Err(_) => continue,
            };
            // SAFETY: vm non-null; name_c is a valid NUL-terminated string.
            let handle = unsafe { cpdev_var(self.vm, name_c.as_ptr()) };
            if handle.is_null() {
                continue; // not addressable; omit from the watch table
            }
            // SAFETY: handle is a live VMVariable* owned by the vm.
            let size = unsafe { cpdev_var_size(handle) };
            if size < 0 {
                continue;
            }
            self.vars.push(VarDecl {
                name: var.name,
                handle,
                size: size as usize,
                ty: var.ty,
            });
        }
        Ok(())
    }

    /// Surface a bad-format status after loading as a diagnostic.
    fn check_format(&self) -> Result<(), Vec<Diagnostic>> {
        // SAFETY: vm is non-null.
        let status = unsafe { cpdev_status1(self.vm) };
        if status & WMSTAT_BADFORMAT != 0 {
            return Err(err(
                "E_XCP_BADFORMAT",
                "loaded program is not valid XCP bytecode".to_owned(),
            ));
        }
        Ok(())
    }
}

impl ExecutionEngine for XcpEngine {
    /// Load by reading the `.XCP` (and its `.dcp` sidecar) from the path in the
    /// document URI. `.XCP` is binary, so the textual `SourceDocument` body is
    /// not used — the engine goes to disk.
    fn load(&mut self, document: &SourceDocument) -> Result<(), Vec<Diagnostic>> {
        let xcp_path = uri_to_path(document.uri());
        self.reset()?;
        let xcp_c = path_to_cstring(&xcp_path)?;
        // SAFETY: vm non-null (reset ran); xcp_c is a valid NUL-terminated path.
        let rc = unsafe { cpdev_load_xcp_file(self.vm, xcp_c.as_ptr()) };
        if rc != 0 {
            return Err(err(
                "E_XCP_LOAD",
                format!("failed to load XCP `{}` (code {rc})", xcp_path.display()),
            ));
        }
        self.load_dcp_and_vars(&xcp_path)
    }

    /// Load the `.XCP` from in-memory bytes; the `.dcp` sidecar is still resolved
    /// from `uri` on disk (it carries the variable map the bytes do not).
    fn load_artifact(&mut self, bytes: &[u8], uri: &str) -> Result<(), Vec<Diagnostic>> {
        let xcp_path = uri_to_path(uri);
        self.reset()?;
        let len = i32::try_from(bytes.len())
            .map_err(|_| err("E_XCP_TOOBIG", "XCP image exceeds 2 GiB".to_owned()))?;
        // SAFETY: vm non-null; bytes/len describe a valid readable slice that the
        // shim copies into its own buffer before returning.
        let rc = unsafe { cpdev_load_xcp(self.vm, bytes.as_ptr(), len) };
        if rc != 0 {
            return Err(err(
                "E_XCP_LOAD",
                format!("failed to load XCP bytes (code {rc})"),
            ));
        }
        self.load_dcp_and_vars(&xcp_path)?;
        self.check_format()
    }

    fn set_scan_interval_ms(&mut self, _scan_interval_ms: i64) {
        // The watch snapshot needs N cycles run deterministically and fast, not
        // wall-clock pacing, so the VM runs free (task_cycle = 0). The requested
        // interval is intentionally ignored.
    }

    fn set_input(&mut self, name: &str, value: &str) {
        if let Some(index) = self.vars.iter().position(|v| v.name == name) {
            let var = &self.vars[index];
            if let Some(bytes) = var.ty.encode(value, var.size) {
                self.staged.retain(|(i, _)| *i != index);
                self.staged.push((index, bytes));
            }
        }
    }

    fn run_scans(&mut self, cycles: u64) {
        if self.vm.is_null() {
            return;
        }
        // SAFETY: vm is non-null below; all handles in `staged`/`vars` are live.
        unsafe {
            cpdev_set_task_cycle(self.vm, 0);
            if !self.initialized {
                cpdev_initialize(self.vm, INIT_MODE);
                self.initialized = true;
            }
            for _ in 0..cycles {
                for (index, bytes) in &self.staged {
                    let var = &self.vars[*index];
                    let len = i32::try_from(bytes.len()).unwrap_or(-1);
                    cpdev_set(self.vm, var.handle, bytes.as_ptr(), len);
                }
                cpdev_run_cycle(self.vm);
                if cpdev_run_mode(self.vm) == 0 {
                    break; // a runtime fault fired
                }
            }
        }
    }

    fn watch(&self) -> Vec<String> {
        if self.vm.is_null() {
            return Vec::new();
        }
        self.vars
            .iter()
            .map(|var| {
                let mut buf = vec![0u8; var.size];
                let len = i32::try_from(var.size).unwrap_or(-1);
                // SAFETY: vm non-null; handle live; buf is `size` bytes, matching len.
                let read = unsafe { cpdev_get(self.vm, var.handle, buf.as_mut_ptr(), len) };
                let value = if read < 0 {
                    "<error>".to_owned()
                } else {
                    var.ty.decode(&buf)
                };
                format!("{} = {}", var.name, value)
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn err(code: &'static str, message: String) -> Vec<Diagnostic> {
    vec![Diagnostic {
        severity: DiagnosticSeverity::Error,
        range: Range::at_start(),
        code,
        message,
    }]
}

/// Turn a document URI into a filesystem path, stripping a `file://` prefix when
/// present (the `plc run` flow builds `file://<abs-path>`).
fn uri_to_path(uri: &str) -> PathBuf {
    let path = uri.strip_prefix("file://").unwrap_or(uri);
    PathBuf::from(path)
}

fn path_to_cstring(path: &Path) -> Result<CString, Vec<Diagnostic>> {
    CString::new(path.to_string_lossy().as_bytes()).map_err(|_| {
        err(
            "E_PATH_NUL",
            format!("path contains an interior NUL: {}", path.display()),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_and_frees_a_vm() {
        // SAFETY: round-trip a handle across the FFI boundary.
        unsafe {
            let vm = cpdev_open();
            assert!(!vm.is_null(), "cpdev_open returned null");
            cpdev_free(vm);
        }
    }

    #[test]
    fn parses_dcp_var_names_in_order() {
        let xml = r#"<GLOBAL>
            <VAR LName="OUT0" PName="WWJ.OUT0" Addr="0000" Size="1" Type="BOOL" />
            <VAR LName="ONOF" PName="WWJ.ONOF" Addr="0004" Size="1" Type="BOOL" />
        </GLOBAL>"#;
        let vars = parse_dcp_vars(xml);
        let names: Vec<&str> = vars.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, ["OUT0", "ONOF"]);
        assert!(vars.iter().all(|v| v.ty == IecType::Bool));
    }

    #[test]
    fn encodes_and_decodes_iec_values() {
        assert_eq!(IecType::Bool.decode(&[1]), "TRUE");
        assert_eq!(IecType::Bool.decode(&[0]), "FALSE");
        assert_eq!(IecType::Signed.decode(&0xFFFFu16.to_le_bytes()), "-1");
        assert_eq!(IecType::Unsigned.decode(&0xFFFFu16.to_le_bytes()), "65535");
        assert_eq!(IecType::Real.decode(&12.0f32.to_le_bytes()), "12.0");
        assert_eq!(IecType::Bool.encode("TRUE", 1), Some(vec![1]));
        assert_eq!(IecType::Signed.encode("-1", 2), Some(vec![0xFF, 0xFF]));
    }
}
