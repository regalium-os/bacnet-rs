// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Answering reads: `ReadProperty` and `ReadPropertyMultiple`.

use super::{error, reject};
use crate::obj::Database;
use crate::service::{
    PropertyReference, PropertyResult, ReadProperty, ReadPropertyAck, ReadPropertyMultiple,
    ReadPropertyMultipleAck, ReadResult, confirmed,
};

/// `all`, `required`, `optional`: each expanded to every property the
/// object has. A database does not say which are required, and a client
/// asking for `required` wants at least those.
const MANY: [u32; 3] = [8, 105, 80];

pub(super) fn read_property(invoke: u8, body: &[u8], db: &dyn Database) -> Vec<u8> {
    let r = match ReadProperty::decode(body) {
        Ok(r) => r,
        Err(e) => return reject(invoke, &e),
    };
    match db.read(r.object, r.property, r.index) {
        Ok(value) => {
            let mut out = vec![0x30, invoke, confirmed::READ_PROPERTY];
            ReadPropertyAck {
                read: r,
                value: &value,
            }
            .encode(&mut out);
            out
        }
        Err(e) => error(invoke, confirmed::READ_PROPERTY, e),
    }
}

pub(super) fn read_multiple(invoke: u8, body: &[u8], db: &dyn Database) -> Vec<u8> {
    let r = match ReadPropertyMultiple::decode(body) {
        Ok(r) => r,
        Err(e) => return reject(invoke, &e),
    };
    // Every value read first, owned, so the ACK can borrow them all.
    let mut asked: Vec<(usize, PropertyReference)> = Vec::new();
    for (i, a) in r.reads.iter().enumerate() {
        for p in &a.properties {
            match (MANY.contains(&p.property), db.properties(a.object)) {
                (true, Ok(all)) => {
                    asked.extend(all.into_iter().map(|q| (i, PropertyReference::new(q))));
                }
                _ => asked.push((i, *p)),
            }
        }
    }
    let values: Vec<_> = asked
        .iter()
        .map(|(i, p)| {
            r.reads
                .get(*i)
                .map(|a| db.read(a.object, p.property, p.index))
        })
        .collect();
    let objects = r
        .reads
        .iter()
        .enumerate()
        .map(|(i, a)| ReadResult {
            object: a.object,
            results: asked
                .iter()
                .zip(&values)
                .filter(|((j, _), _)| *j == i)
                .filter_map(|((_, p), v)| {
                    let v = v.as_ref()?;
                    Some(PropertyResult {
                        property: *p,
                        result: v.as_ref().map(Vec::as_slice).map_err(|e| *e),
                    })
                })
                .collect(),
        })
        .collect();
    let mut out = vec![0x30, invoke, confirmed::READ_PROPERTY_MULTIPLE];
    ReadPropertyMultipleAck { objects }.encode(&mut out);
    out
}
