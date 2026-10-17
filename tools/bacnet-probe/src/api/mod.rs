// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The `BACnet` API the schema declares — `bacnet.device.v1` and
//! `bacnet.object.v1` — served over this probe's client.
//!
//! Each method takes the request message and returns the response message
//! the .proto names, so putting a transport in front of it is an adapter
//! that moves bytes, not a second implementation. Every call is a `BACnet`
//! request to the device; nothing is answered from a cache except the list
//! of devices discovery has found, which is what `ListDevices` is.
//!
//! # Concurrency
//!
//! `Send`, so it can live on a worker thread, and every method takes
//! `&mut self`: it owns one socket, and replies are matched on it in order.
//! Share it through a channel to its thread, never a lock held across calls.

mod device;
mod error;
mod object;
mod object_write;
mod property;
mod value;

pub mod name;

pub use error::{ApiError, Code};
pub use value::{from_schema, to_schema, values_to_schema};

use crate::{Client, Device, Discovery};
use std::collections::BTreeMap;
use std::time::SystemTime;

/// A device discovery has found, and what this API has learnt of it.
struct Known {
    device: Device,
    /// Its object-name, once read.
    name: String,
    discovered: SystemTime,
    seen: SystemTime,
    reachable: bool,
}

/// The API, over one client. See the module documentation.
pub struct Api {
    client: Client,
    plan: Discovery,
    known: BTreeMap<u32, Known>,
}

impl Api {
    /// Serves over `client`, discovering with `plan`. Sends nothing.
    #[must_use]
    pub fn new(client: Client, plan: Discovery) -> Self {
        Self {
            client,
            plan,
            known: BTreeMap::new(),
        }
    }

    /// The client underneath, for a caller that also follows values live.
    pub fn client(&mut self) -> &mut Client {
        &mut self.client
    }

    /// A device by instance: one discovery has found, or else one a ranged
    /// Who-Is finds now.
    fn device(&mut self, instance: u32) -> Result<Device, ApiError> {
        if let Some(k) = self.known.get(&instance) {
            return Ok(k.device.clone());
        }
        let plan = self.plan.clone().with_range(Some((instance, instance)));
        let found = self
            .client
            .discover(&plan)
            .map_err(|e| ApiError::new(Code::Unavailable, format!("Who-Is: {e}")))?;
        let device = found.into_iter().next().ok_or_else(|| {
            ApiError::new(
                Code::NotFound,
                format!("device {instance} did not answer Who-Is"),
            )
        })?;
        self.remember(device.clone());
        Ok(device)
    }

    fn remember(&mut self, device: Device) {
        let now = SystemTime::now();
        self.known
            .entry(device.instance)
            .and_modify(|k| {
                k.device = device.clone();
                k.seen = now;
                k.reachable = true;
            })
            .or_insert(Known {
                device,
                name: String::new(),
                discovered: now,
                seen: now,
                reachable: true,
            });
    }

    /// Records whether `instance` answered, from the outcome of a call.
    fn answered<T>(&mut self, instance: u32, result: &Result<T, ApiError>) {
        if let Some(k) = self.known.get_mut(&instance) {
            let silent = matches!(result, Err(e) if e.code == Code::Unavailable);
            k.reachable = !silent;
            if !silent {
                k.seen = SystemTime::now();
            }
        }
    }
}

/// Where a listing resumes: the offset an earlier page ended at.
fn page(token: &str, size: i32, max: usize) -> Result<(usize, usize), ApiError> {
    let start = if token.is_empty() {
        0
    } else {
        token.parse().map_err(|_| {
            ApiError::invalid(format!("{token:?} is not a page token from this API"))
        })?
    };
    let size = usize::try_from(size).unwrap_or(0);
    Ok((
        start,
        if size == 0 {
            50.min(max)
        } else {
            size.min(max)
        },
    ))
}

/// The next page's token, or empty when `end` is the last.
fn next_token(end: usize, total: usize) -> String {
    if end < total {
        end.to_string()
    } else {
        String::new()
    }
}

/// Rejects a filter rather than ignoring it: a listing that silently
/// returned everything would look like an answer.
fn no_filter(filter: &str) -> Result<(), ApiError> {
    if filter.is_empty() {
        Ok(())
    } else {
        Err(ApiError::new(
            Code::Unimplemented,
            "the probe does not evaluate AIP-160 filters yet",
        ))
    }
}
