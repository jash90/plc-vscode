//! Data-segment allocator. CPDev data memory is a single flat little-endian
//! region addressed by byte offset; this hands out non-overlapping offsets for
//! globals, constants, and compiler temporaries.

use std::collections::HashMap;

/// The VM addresses data with 16-bit offsets (`MaxDataAddress = 0xFFFF`), so the
/// data segment can be up to 64 KiB. The `plc_cpdev_vm` shim sizes its data
/// buffer from the `.DCP` and floors it at the vendored 256-byte default, so a
/// program is bounded only by this address space, not the old static buffer.
pub const DEFAULT_DATA_CAP: u16 = u16::MAX;

/// Assigns byte offsets in the data segment.
#[derive(Debug)]
pub struct DataLayout {
    cursor: u16,
    cap: u16,
    named: HashMap<String, u16>,
}

impl DataLayout {
    /// A fresh layout starting at offset 0 with the given byte capacity.
    pub fn new(cap: u16) -> Self {
        Self {
            cursor: 0,
            cap,
            named: HashMap::new(),
        }
    }

    /// Allocate (or return the existing offset of) a named slot of `size` bytes.
    pub fn alloc(&mut self, name: &str, size: usize) -> Result<u16, String> {
        if let Some(&addr) = self.named.get(name) {
            return Ok(addr);
        }
        let addr = self.bump(name, size)?;
        self.named.insert(name.to_owned(), addr);
        Ok(addr)
    }

    /// Allocate an anonymous slot of `size` bytes (a constant or temporary).
    pub fn alloc_anon(&mut self, size: usize) -> Result<u16, String> {
        self.bump("<temp>", size)
    }

    /// Total bytes allocated so far (the data-segment size).
    pub fn size(&self) -> u16 {
        self.cursor
    }

    fn bump(&mut self, what: &str, size: usize) -> Result<u16, String> {
        // A zero-size slot (e.g. an unsized STRING sentinel) is a codegen bug here.
        let size = u16::try_from(size.max(1))
            .map_err(|_| format!("slot `{what}` size {size} exceeds 16-bit data space"))?;
        let addr = self.cursor;
        let next = self
            .cursor
            .checked_add(size)
            .filter(|&n| n <= self.cap)
            .ok_or_else(|| {
                format!(
                    "data segment overflow allocating `{what}` ({size} bytes at {addr}); cap is {} bytes",
                    self.cap
                )
            })?;
        self.cursor = next;
        Ok(addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_slots_are_stable_and_packed() {
        let mut l = DataLayout::new(DEFAULT_DATA_CAP);
        assert_eq!(l.alloc("a", 1).unwrap(), 0);
        assert_eq!(l.alloc("b", 2).unwrap(), 1);
        assert_eq!(l.alloc("a", 1).unwrap(), 0); // stable
        assert_eq!(l.alloc("c", 4).unwrap(), 3);
        assert_eq!(l.size(), 7);
    }

    #[test]
    fn overflow_errors() {
        let mut l = DataLayout::new(4);
        assert_eq!(l.alloc("a", 2).unwrap(), 0);
        assert!(l.alloc("b", 4).is_err());
    }
}
