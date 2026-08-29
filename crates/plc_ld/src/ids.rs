//! Stable element identity for LD programs.
//!
//! [`normalize_ids`] assigns deterministic `r{n}` ids to rungs and `e{n}` ids
//! to contacts/output elements, preserving valid existing ids and resolving
//! collisions. Editors call it on load (after parsing a v1 file) and again
//! before save so ids stay stable across sessions while remaining unique.
//!
//! Ids are cosmetic to execution: [`crate::lower`] ignores them entirely.

use std::collections::HashSet;

use crate::model::{LdProgram, OutputElement};

/// Assign stable, unique ids to every rung and element of `program`.
///
/// - Rungs receive `r0`, `r1`, … in document order.
/// - Contacts and output elements receive `e0`, `e1`, … in document order
///   (branch contacts first, then outputs, per rung).
/// - Existing non-empty ids with the right prefix are preserved; a duplicate
///   claim keeps the first occurrence and regenerates the rest.
/// - Fresh counters are seeded past the highest numeric suffix already in
///   use for that prefix, so newly assigned ids never collide.
pub fn normalize_ids(program: &mut LdProgram) {
    let mut rungs = Ids::new("r", program);
    let mut elements = Ids::new("e", program);

    for rung in &mut program.rungs {
        rungs.ensure(&mut rung.id);
        for branch in &mut rung.branches {
            for contact in &mut branch.elements {
                elements.ensure(&mut contact.id);
            }
        }
        for output in &mut rung.outputs {
            match output {
                OutputElement::Coil { id, .. } => elements.ensure(id),
                OutputElement::Block { id, .. } => elements.ensure(id),
            }
        }
    }
}

/// Id allocation state for one prefix (`r` or `e`).
struct Ids {
    prefix: &'static str,
    next: u32,
    used: HashSet<String>,
}

impl Ids {
    /// Seed the counter past the highest numeric `<prefix><n>` id in the
    /// program. `used` starts empty: keep-decisions are made during the walk,
    /// in document order, via insert-result.
    fn new(prefix: &'static str, program: &LdProgram) -> Self {
        let mut next = 0;
        let mut consider = |id: &Option<String>| {
            if let Some(id) = id
                && let Some(suffix) = id.strip_prefix(prefix)
                && let Ok(n) = suffix.parse::<u32>()
                && n >= next
            {
                next = n + 1;
            }
        };
        for rung in &program.rungs {
            consider(&rung.id);
            for branch in &rung.branches {
                for contact in &branch.elements {
                    consider(&contact.id);
                }
            }
            for output in &rung.outputs {
                match output {
                    OutputElement::Coil { id, .. } => consider(id),
                    OutputElement::Block { id, .. } => consider(id),
                }
            }
        }
        Ids {
            prefix,
            next,
            used: HashSet::new(),
        }
    }

    /// Keep a valid, unused id; regenerate everything else.
    fn ensure(&mut self, slot: &mut Option<String>) {
        let keep = match slot {
            Some(id) if is_valid(id, self.prefix) => self.used.insert(id.clone()),
            _ => false,
        };
        if !keep {
            *slot = Some(self.allocate());
        }
    }

    fn allocate(&mut self) -> String {
        loop {
            let candidate = format!("{}{}", self.prefix, self.next);
            self.next += 1;
            if self.used.insert(candidate.clone()) {
                return candidate;
            }
        }
    }
}

/// A valid id is non-empty beyond the prefix and uses the expected prefix.
fn is_valid(id: &str, prefix: &str) -> bool {
    id.len() > prefix.len() && id.starts_with(prefix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CoilVariant, ContactElement, OutputElement, Rung, SeriesBranch};

    fn rung_with(ids: &[Option<&str>]) -> Rung {
        Rung {
            id: None,
            comment: None,
            branches: vec![SeriesBranch {
                elements: ids
                    .iter()
                    .map(|id| ContactElement {
                        id: id.map(str::to_owned),
                        name: "X".to_owned(),
                        negated: false,
                    })
                    .collect(),
            }],
            outputs: vec![OutputElement::Coil {
                id: None,
                name: "Out".to_owned(),
                variant: CoilVariant::Normal,
            }],
        }
    }

    #[test]
    fn normalize_assigns_stable_ids() {
        let mut program = LdProgram::new("P");
        program.rungs.push(rung_with(&[None, None]));
        normalize_ids(&mut program);
        assert_eq!(program.rungs[0].id.as_deref(), Some("r0"));
        assert_eq!(
            program.rungs[0].branches[0].elements[0].id.as_deref(),
            Some("e0")
        );
        assert_eq!(
            program.rungs[0].branches[0].elements[1].id.as_deref(),
            Some("e1")
        );
    }

    #[test]
    fn normalize_preserves_existing_ids() {
        let mut program = LdProgram::new("P");
        program.rungs.push(rung_with(&[Some("e-keep")]));
        normalize_ids(&mut program);
        assert_eq!(
            program.rungs[0].branches[0].elements[0].id.as_deref(),
            Some("e-keep")
        );
    }

    #[test]
    fn normalize_resolves_collisions() {
        let mut program = LdProgram::new("P");
        program.rungs.push(rung_with(&[Some("e0"), Some("e0")]));
        normalize_ids(&mut program);
        let ids: Vec<&str> = program.rungs[0].branches[0]
            .elements
            .iter()
            .map(|c| c.id.as_deref().unwrap())
            .collect();
        assert_eq!(ids[0], "e0");
        assert_ne!(ids[1], "e0");
    }

    #[test]
    fn next_id_skips_used() {
        let mut program = LdProgram::new("P");
        program.rungs.push(rung_with(&[Some("e7"), None]));
        normalize_ids(&mut program);
        assert_eq!(
            program.rungs[0].branches[0].elements[0].id.as_deref(),
            Some("e7")
        );
        assert_eq!(
            program.rungs[0].branches[0].elements[1].id.as_deref(),
            Some("e8")
        );
    }
}
