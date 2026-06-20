//! The CPDev VM instruction catalog, parsed from the vendored VMSpec XML.
//!
//! This is the "generated from spec" core: codegen looks opcodes up here by
//! `(function name, operand type)` rather than computing them, because the spec
//! enumerates each typed variant explicitly — including the irregular ones (e.g.
//! `NOT` sub-enumerates its type nibble, so `NOT BYTE` = `0x0511`, which no
//! nibble formula would produce).
//!
//! The six XML descriptors (`VM-Univ.xml` is the master that `INCLUDE`s the
//! others) are bundled with `include_str!`; [`SpecTable::load`] parses the
//! `<function>` and `<sysproc>` elements into a flat catalog.
//!
//! Caveat: variant lookup keys on the *first value argument's* type, which is the
//! operand type for arithmetic / boolean / comparison families. `SEL`/`MUX` lead
//! with a selector argument, so their value type is not the first arg — they need
//! dedicated handling and must not be fetched via [`SpecTable::typed`].

use std::collections::HashMap;

use crate::types::CpType;

const VM_CORE: &str = include_str!("../spec_xml/VMCore.xml");
const LREALS: &str = include_str!("../spec_xml/lreals.xml");
const LE_IF: &str = include_str!("../spec_xml/le-IF.xml");
const FLASH: &str = include_str!("../spec_xml/flash.xml");
const STRINGS: &str = include_str!("../spec_xml/strings.xml");
const VM_UNIV: &str = include_str!("../spec_xml/VM-Univ.xml");

/// A 16-bit vmcode, possibly with a variadic count placeholder (`*` in the XML,
/// `F` in `vmdef.h`) at bits 4-7.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vmcode {
    /// The opcode with the variadic count nibble cleared to 0.
    pub base: u16,
    /// Whether bits 4-7 are a source-operand count placeholder.
    pub variadic: bool,
}

impl Vmcode {
    /// Parse a 4-hex-digit vmcode string (e.g. `"1C15"`, `"01*2"`).
    fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.len() != 4 {
            return None;
        }
        let mut base: u16 = 0;
        let mut variadic = false;
        for (i, ch) in text.chars().enumerate() {
            let nibble = if ch == '*' {
                // The variadic placeholder is always the bits-4-7 nibble (index 2).
                if i != 2 {
                    return None;
                }
                variadic = true;
                0
            } else {
                ch.to_digit(16)? as u16
            };
            base = (base << 4) | nibble;
        }
        Some(Self { base, variadic })
    }

    /// The concrete opcode for the given source-operand count (ignored when the
    /// op is not variadic).
    pub fn encode(self, count: u8) -> u16 {
        if self.variadic {
            self.base | ((u16::from(count) & 0xF) << 4)
        } else {
            self.base
        }
    }
}

/// One catalog entry: a function or system procedure with its opcode and the
/// type of its first value operand (`None` for sysprocs / nullary functions).
#[derive(Debug, Clone)]
struct Entry {
    vmcode: Vmcode,
    operand: Option<CpType>,
}

/// Normalize a type for opcode lookup: all `STRING[N]` collapse to one key, since
/// the VM has a single opcode per string operation regardless of capacity.
fn canon(t: CpType) -> CpType {
    match t {
        CpType::Str(_) => CpType::Str(0),
        other => other,
    }
}

/// The parsed instruction catalog, keyed by uppercased function name.
#[derive(Debug, Clone, Default)]
pub struct SpecTable {
    by_name: HashMap<String, Vec<Entry>>,
}

impl SpecTable {
    /// Parse the bundled VMSpec XML into the catalog.
    pub fn load() -> Self {
        let mut table = SpecTable::default();
        for xml in [VM_CORE, LREALS, LE_IF, FLASH, STRINGS, VM_UNIV] {
            table.ingest(xml, "function");
            table.ingest(xml, "sysproc");
        }
        table
    }

    fn ingest(&mut self, xml: &str, tag: &str) {
        for (start_tag, body) in elements(xml, tag) {
            let (Some(name), Some(vmcode)) = (
                attr(start_tag, "name"),
                attr(start_tag, "vmcode").and_then(|v| Vmcode::parse(&v)),
            ) else {
                continue;
            };
            self.by_name
                .entry(name.to_ascii_uppercase())
                .or_default()
                .push(Entry {
                    vmcode,
                    operand: first_value_arg_type(body),
                });
        }
    }

    /// Look up a typed op variant by `(name, operand type)` — for arithmetic,
    /// boolean, unary, and comparison families (not `SEL`/`MUX`; see module docs).
    /// STRING capacity is ignored in the match (there is one opcode per string
    /// op regardless of declared `STRING[N]` capacity).
    pub fn typed(&self, name: &str, operand: CpType) -> Option<Vmcode> {
        let want = canon(operand);
        self.by_name
            .get(&name.to_ascii_uppercase())?
            .iter()
            .find(|e| e.operand.map(canon) == Some(want))
            .map(|e| e.vmcode)
    }

    /// Look up an opcode that has no type variants: system procedures (`MCD`,
    /// `JZ`, `CALB`, ...) and fixed functions (`CUR_TIME`). These have exactly one
    /// catalog entry; a name with several typed variants returns `None` here (use
    /// [`SpecTable::typed`]).
    pub fn untyped(&self, name: &str) -> Option<Vmcode> {
        match self.by_name.get(&name.to_ascii_uppercase())?.as_slice() {
            [only] => Some(only.vmcode),
            _ => None,
        }
    }
}

/// Yield `(start_tag, body)` for every `<tag ...> … </tag>` (or self-closing
/// `<tag .../>`, with an empty body) element in `xml`.
fn elements<'a>(xml: &'a str, tag: &str) -> Vec<(&'a str, &'a str)> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = Vec::new();
    let mut base = 0usize;
    while let Some(rel) = xml[base..].find(&open) {
        let start = base + rel;
        // The character after the tag name must be whitespace or `>`/`/` so we
        // don't match a longer tag that happens to share the prefix.
        let after = xml[start + open.len()..].chars().next();
        if !matches!(after, Some(c) if c.is_whitespace() || c == '>' || c == '/') {
            base = start + open.len();
            continue;
        }
        let Some(tag_end_rel) = xml[start..].find('>') else {
            break;
        };
        let tag_end = start + tag_end_rel;
        let start_tag = &xml[start..tag_end];
        if start_tag.ends_with('/') {
            out.push((start_tag, ""));
            base = tag_end + 1;
            continue;
        }
        let Some(body_end_rel) = xml[tag_end..].find(&close) else {
            break;
        };
        let body = &xml[tag_end + 1..tag_end + body_end_rel];
        out.push((start_tag, body));
        base = tag_end + body_end_rel + close.len();
    }
    out
}

/// The type of the first `<arg>` whose `type` is a plain IEC type name (not an
/// addressing-mode `:imm.*`/`:rdlabel`/... operand). That is the operand type for
/// the arithmetic/boolean/comparison families.
fn first_value_arg_type(body: &str) -> Option<CpType> {
    let mut rest = body;
    while let Some(rel) = rest.find("<arg") {
        rest = &rest[rel + 4..];
        let tag_end = rest.find('>').unwrap_or(rest.len());
        let tag = &rest[..tag_end];
        rest = &rest[tag_end..];
        if let Some(ty) = attr(tag, "type")
            && !ty.starts_with(':')
            && let Some(cp) = CpType::from_name(&ty)
        {
            return Some(cp);
        }
    }
    None
}

/// Read a `key="value"` attribute out of an XML start-tag's text. The match must
/// be a whole attribute name (preceded by start-of-tag or whitespace).
fn attr(tag: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=\"");
    let mut search = tag;
    while let Some(pos) = search.find(&needle) {
        let preceded_ok = pos == 0 || search.as_bytes()[pos - 1].is_ascii_whitespace();
        let after = &search[pos + needle.len()..];
        if preceded_ok {
            let end = after.find('"')?;
            return Some(after[..end].to_owned());
        }
        search = after;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vmcode_parses_concrete_and_variadic() {
        assert_eq!(
            Vmcode::parse("1C15"),
            Some(Vmcode {
                base: 0x1C15,
                variadic: false
            })
        );
        let add = Vmcode::parse("01*2").unwrap();
        assert!(add.variadic);
        assert_eq!(add.base, 0x0102);
        assert_eq!(add.encode(2), 0x0122); // count nibble filled
        assert_eq!(add.encode(3), 0x0132);
    }

    /// Cross-check the parsed spec against the authoritative vendored `vmdef.h`
    /// constants (and the encodings the byte-exact fixture test proved).
    #[test]
    fn matches_vmdef_constants() {
        let t = SpecTable::load();

        // Regular typed families: low nibble == type code.
        assert_eq!(t.typed("EQ", CpType::Int).unwrap().encode(0), 0x1202);
        assert_eq!(t.typed("SUB", CpType::Time).unwrap().encode(0), 0x020B);
        assert_eq!(t.typed("LT", CpType::Int).unwrap().encode(0), 0x1402);
        assert_eq!(t.typed("GE", CpType::Time).unwrap().encode(0), 0x110B);
        assert_eq!(t.typed("GT", CpType::Real).unwrap().encode(0), 0x1009);

        // Variadic families.
        assert_eq!(t.typed("ADD", CpType::Int).unwrap().encode(2), 0x0122);
        assert_eq!(t.typed("MUL", CpType::Int).unwrap().encode(2), 0x0322);
        assert_eq!(t.typed("AND", CpType::Bool).unwrap().encode(2), 0x0820);

        // Irregular sub-enumerated nibble — proves operand-type keying, not a
        // nibble formula: NOT BYTE is 0x0511, not 0x05_5.
        assert_eq!(t.typed("NOT", CpType::Bool).unwrap().encode(0), 0x0510);
        assert_eq!(t.typed("NOT", CpType::Byte).unwrap().encode(0), 0x0511);
        assert_eq!(t.typed("NOT", CpType::Word).unwrap().encode(0), 0x0512);

        // System procedures + nullary functions (looked up by name only).
        assert_eq!(t.untyped("MCD").unwrap().encode(0), 0x1C15);
        assert_eq!(t.untyped("CALB").unwrap().encode(0), 0x1C16);
        assert_eq!(t.untyped("TRML").unwrap().encode(0), 0x1C1D);
        assert_eq!(t.untyped("RETURN").unwrap().encode(0), 0x1C13);
        assert_eq!(t.untyped("JZ").unwrap().encode(0), 0x1C02);
        assert_eq!(t.untyped("JMP").unwrap().encode(0), 0x1C00);
        assert_eq!(t.untyped("MEMCP").unwrap().encode(0), 0x1C1F);
        assert_eq!(t.untyped("CUR_TIME").unwrap().encode(0), 0x1C17);
    }
}
