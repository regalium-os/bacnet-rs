// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The four services that change the simulated gateway, answered as a
//! controller with a dynamic object database would answer them.
//!
//! Objects it was shipped with cannot be deleted, as a real gateway's own
//! points cannot; objects created at runtime can. Analog, binary and
//! multi-state values are creatable, which is what a commissioning tool
//! adds to a controller.

use super::{DEVICE, Sim};
use crate::service::object::{self, Specifier};
use crate::service::write::{self, PropertyValue};
use crate::service::{self, confirmed};
use crate::tag::{self, ObjectId, Value};

/// Object types the simulator creates: analog-value, binary-value,
/// multi-state-value.
const CREATABLE: &[u16] = &[2, 5, 19];

impl Sim {
    /// The reply to a write, create or delete; `None` when `choice` is
    /// none of those.
    pub(super) fn change(&mut self, invoke: u8, choice: u8, body: &[u8]) -> Option<Vec<u8>> {
        let malformed = service::reject(invoke, 4);
        let ack = service::simple_ack(invoke, choice);
        let err = |(class, code): (u32, u32)| service::error(invoke, choice, class, code);
        Some(match choice {
            confirmed::WRITE_PROPERTY => match write::decode_request(body) {
                Some((object, w)) => self.write(object, &w).map_or_else(err, |()| ack),
                None => malformed,
            },
            confirmed::WRITE_PROPERTY_MULTIPLE => match write::decode_request_multiple(body) {
                Some(writes) => self.write_all(invoke, &writes).unwrap_or(ack),
                None => malformed,
            },
            confirmed::CREATE_OBJECT => match object::decode_create_request(body) {
                Some((what, initial)) => self.create(invoke, what, &initial),
                None => malformed,
            },
            confirmed::DELETE_OBJECT => match object::decode_object_body(body) {
                Some(id) => self.delete(id).map_or_else(err, |()| ack),
                None => malformed,
            },
            _ => return None,
        })
    }

    /// Applies writes in order, stopping at the first refusal (§15.10.1.3.1).
    fn write_all(
        &mut self,
        invoke: u8,
        writes: &[(ObjectId, Vec<PropertyValue>)],
    ) -> Option<Vec<u8>> {
        for (object, values) in writes {
            for w in values {
                if let Err((class, code)) = self.write(*object, w) {
                    // firstFailedWriteAttempt: [1] { [0] object [1] property }.
                    let mut failed = Vec::new();
                    tag::push_opening(&mut failed, 1);
                    tag::push_object(&mut failed, 0, true, *object);
                    tag::push_unsigned(&mut failed, 1, true, w.property.property.into());
                    tag::push_closing(&mut failed, 1);
                    let body = object::constructed_error(class, code, &failed);
                    return Some(
                        [
                            &[0x50, invoke, confirmed::WRITE_PROPERTY_MULTIPLE][..],
                            &body,
                        ]
                        .concat(),
                    );
                }
            }
        }
        None
    }

    fn create(&mut self, invoke: u8, what: Specifier, initial: &[PropertyValue]) -> Vec<u8> {
        let refuse = |class: u32, code: u32, element: u64| {
            let mut failed = Vec::new();
            tag::push_unsigned(&mut failed, 1, true, element);
            let body = object::constructed_error(class, code, &failed);
            [&[0x50, invoke, confirmed::CREATE_OBJECT][..], &body].concat()
        };
        let id = match what {
            Specifier::Id(id) if self.props.contains_key(&(id, 77)) => return refuse(1, 24, 0),
            Specifier::Id(id) => id,
            Specifier::Type(kind) => {
                let next = self
                    .props
                    .keys()
                    .filter(|(o, _)| o.kind == kind)
                    .map(|(o, _)| o.instance)
                    .max();
                ObjectId::new(kind, next.map_or(1, |n| n + 1))
            }
        };
        if !CREATABLE.contains(&id.kind) {
            return refuse(1, 36, 0); // object: unsupported-object-type
        }
        self.add_object(id);
        for (n, w) in (1u64..).zip(initial) {
            if let Err((class, code)) = self.write(id, w) {
                let _ = self.delete(id);
                return refuse(class, code, n);
            }
        }
        service::complex_ack(invoke, confirmed::CREATE_OBJECT, &object::object_body(id))
    }

    /// A new object with every property its type requires, at its defaults.
    fn add_object(&mut self, id: ObjectId) {
        let (value, extra): (Value, Vec<(u32, Vec<Value>)>) = match id.kind {
            2 => (Value::Real(0.0), vec![(117, vec![Value::Enumerated(95)])]), // no-units
            5 => (
                Value::Enumerated(0),
                vec![
                    (4, vec![Value::Text("active".into())]),
                    (46, vec![Value::Text("inactive".into())]),
                ],
            ),
            _ => (
                Value::Unsigned(1),
                vec![
                    (74, vec![Value::Unsigned(2)]),
                    (
                        110,
                        vec![Value::Text("State 1".into()), Value::Text("State 2".into())],
                    ),
                ],
            ),
        };
        let name = format!("{} {}", crate::names::object_type(id.kind), id.instance);
        let mut props = vec![
            (77, vec![Value::Text(name)]),
            (75, vec![Value::Object(id)]),
            (79, vec![Value::Enumerated(id.kind.into())]),
            (85, vec![value]),
            (111, vec![Value::Bits(vec![false; 4])]),
            (36, vec![Value::Enumerated(0)]),
            (81, vec![Value::Bool(false)]),
        ];
        props.extend(extra);
        for (p, v) in props {
            self.props.insert((id, p), v);
        }
        self.created.insert(id);
        self.pinned.insert(id);
        if let Some(list) = self.props.get_mut(&(DEVICE, 76)) {
            list.push(Value::Object(id));
        }
    }

    fn delete(&mut self, id: ObjectId) -> Result<(), (u32, u32)> {
        if !self.props.contains_key(&(id, 77)) {
            return Err((1, 31)); // object: unknown-object
        }
        if !self.created.remove(&id) {
            return Err((1, 23)); // object: object-deletion-not-permitted
        }
        self.props.retain(|(o, _), _| *o != id);
        self.subs.retain(|s| s.object() != id);
        if let Some(list) = self.props.get_mut(&(DEVICE, 76)) {
            list.retain(|v| *v != Value::Object(id));
        }
        Ok(())
    }
}
