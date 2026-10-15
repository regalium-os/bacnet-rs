// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The simulator's replies, built with the same codecs the client decodes.

use super::{DEVICE, Sim};
use crate::frame::Incoming;
use crate::service::rpm::{self, Results};
use crate::service::who::{self, IAm, IHave, Wanted};
use crate::service::{self, PropertyRef, PropertyResult, confirmed, cov, read, unconfirmed};
use crate::tag::{ObjectId, Value};

impl Sim {
    /// Every APDU to send back for `msg`, in order.
    pub(super) fn answer(&mut self, msg: &Incoming<'_>) -> Vec<Vec<u8>> {
        match msg.apdu {
            [0x10, unconfirmed::WHO_IS, body @ ..] => {
                let Some(range) = who::decode_who_is(body) else {
                    return Vec::new();
                };
                if range.is_some_and(|(l, h)| !(l..=h).contains(&DEVICE.instance)) {
                    return Vec::new();
                }
                let me = IAm {
                    device: DEVICE,
                    max_apdu: 1476,
                    segmentation: 3,
                    vendor: 999,
                };
                vec![service::unconfirmed_request(
                    unconfirmed::I_AM,
                    &who::i_am(&me),
                )]
            }
            [0x10, unconfirmed::WHO_HAS, body @ ..] => {
                let Some((_, wanted)) = who::decode_who_has(body) else {
                    return Vec::new();
                };
                self.find(&wanted)
                    .map(|have| {
                        service::unconfirmed_request(unconfirmed::I_HAVE, &who::i_have(&have))
                    })
                    .into_iter()
                    .collect()
            }
            [first, _, invoke, choice, body @ ..] if first >> 4 == 0 => {
                self.confirmed(msg, *invoke, *choice, body)
            }
            _ => Vec::new(),
        }
    }

    fn confirmed(
        &mut self,
        msg: &Incoming<'_>,
        invoke: u8,
        choice: u8,
        body: &[u8],
    ) -> Vec<Vec<u8>> {
        let malformed = || vec![service::reject(invoke, 4)]; // invalid-tag
        match choice {
            confirmed::READ_PROPERTY => match read::decode_request(body) {
                Some((object, p)) => vec![match self.read(object, p) {
                    Ok(values) => {
                        service::complex_ack(invoke, choice, &read::ack(object, p, &values))
                    }
                    Err((class, code)) => service::error(invoke, choice, class, code),
                }],
                None => malformed(),
            },
            confirmed::READ_PROPERTY_MULTIPLE if self.rpm => match rpm::decode_request(body) {
                Some(specs) => {
                    let results: Vec<Results> = specs
                        .iter()
                        .map(|s| Results {
                            object: s.object,
                            properties: s
                                .properties
                                .iter()
                                .map(|p| (*p, self.read(s.object, *p)))
                                .collect(),
                        })
                        .collect();
                    vec![service::complex_ack(invoke, choice, &rpm::ack(&results))]
                }
                None => malformed(),
            },
            confirmed::SUBSCRIBE_COV if self.cov => match cov::decode_subscribe(body) {
                Some(s) if !self.props.contains_key(&(s.object, 77)) => {
                    vec![service::error(invoke, choice, 1, 31)]
                }
                Some(s) => {
                    let mut replies = vec![service::simple_ack(invoke, choice)];
                    replies.extend(self.subscribe(msg.from, s));
                    replies
                }
                None => malformed(),
            },
            _ => vec![service::reject(invoke, 9)], // unrecognized-service
        }
    }

    /// One property, as a device would answer it.
    pub(super) fn read(&self, object: ObjectId, p: PropertyRef) -> PropertyResult {
        if !self.props.contains_key(&(object, 77)) {
            return Err((1, 31)); // object: unknown-object
        }
        let mut values = self
            .props
            .get(&(object, p.property))
            .ok_or((2, 32))?
            .clone();
        if p.property == 85 {
            values = self.present(object).into_iter().collect();
        }
        match p.index {
            None => Ok(values),
            Some(_) if !matches!(p.property, 76 | 87 | 110) => Err((2, 50)), // property-is-not-an-array
            Some(0) => Ok(vec![Value::Unsigned(values.len() as u64)]),
            Some(i) => values
                .get(i as usize - 1)
                .cloned()
                .map(|v| vec![v])
                .ok_or((2, 42)),
        }
    }

    fn find(&self, wanted: &Wanted) -> Option<IHave> {
        let name_of = |id: ObjectId| match self.props.get(&(id, 77))?.first()? {
            Value::Text(s) => Some(s.clone()),
            _ => None,
        };
        let object = match wanted {
            Wanted::Object(id) => *id,
            Wanted::Name(name) => *self
                .props
                .keys()
                .find(|(id, p)| *p == 77 && name_of(*id).as_ref() == Some(name))
                .map(|(id, _)| id)?,
        };
        Some(IHave {
            device: DEVICE,
            object,
            name: name_of(object)?,
        })
    }
}
