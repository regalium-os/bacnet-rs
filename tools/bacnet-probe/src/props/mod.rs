// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Property identifiers (ASHRAE 135 §21, `BACnetPropertyIdentifier`), what
//! each one means to someone reading a live device, and which ones are worth
//! asking each object type for.

mod table;

/// One property: its wire number, its standard name, and why you would care.
pub struct Prop {
    /// `BACnetPropertyIdentifier`.
    pub id: u32,
    /// The standard name, e.g. `present-value`.
    pub name: &'static str,
    /// What it tells someone reading a live device.
    pub meaning: &'static str,
}

pub(crate) const fn p(id: u32, name: &'static str, meaning: &'static str) -> Prop {
    Prop { id, name, meaning }
}

pub use table::ALL;

/// The table entry for `id`.
#[must_use]
pub fn find(id: u32) -> Option<&'static Prop> {
    ALL.iter().find(|p| p.id == id)
}

#[allow(clippy::doc_markdown)]
/// The standard name, or `property-N` for one this table does not know.
#[must_use]
pub fn name(id: u32) -> String {
    find(id).map_or_else(|| format!("property-{id}"), |p| p.name.to_owned())
}

/// Parses a property by standard name (`present-value`) or number (`85`).
#[must_use]
pub fn parse(s: &str) -> Option<u32> {
    s.parse().ok().or_else(|| {
        ALL.iter()
            .find(|p| p.name.eq_ignore_ascii_case(s))
            .map(|p| p.id)
    })
}

/// Properties to read for `--detail`, in display order. Optional properties
/// a device does not implement come back as unknown-property and are skipped.
#[must_use]
pub fn for_kind(kind: u16) -> Vec<u32> {
    let mut list = vec![77, 79, 28];
    if kind == 8 {
        list.extend([
            121, 120, 70, 44, 12, 58, 112, 98, 139, 155, 62, 107, 11, 73, 56, 57, 97, 96,
        ]);
    } else {
        list.extend([85, 111, 36, 103, 81]);
        match kind {
            0..=2 => list.extend([117, 106, 22, 69, 65]),
            3..=5 => list.extend([4, 46, 84]),
            13 | 14 | 19 => list.extend([74, 110]),
            _ => {}
        }
        // Outputs are always commandable; values may be. Ask either way.
        if matches!(kind, 1 | 2 | 4 | 5 | 14 | 19) {
            list.extend([87, 104]);
        }
    }
    list
}

/// Properties whose value is a `BACnetARRAY`, read whole or element by element.
#[must_use]
pub fn is_array(id: u32) -> bool {
    matches!(id, 76 | 87 | 110)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_property_in_a_detail_list_has_a_name_and_a_meaning() {
        for kind in [0, 1, 3, 4, 8, 13, 19, 59] {
            for id in for_kind(kind) {
                assert!(
                    find(id).is_some(),
                    "property {id} (object type {kind}) is not in ALL"
                );
            }
        }
    }

    #[test]
    fn the_table_has_no_duplicate_ids() {
        let mut ids: Vec<u32> = ALL.iter().map(|p| p.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), ALL.len());
    }
}
