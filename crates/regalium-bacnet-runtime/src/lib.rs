// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The long-lived half of the driver: the thing that actually waits.
//!
//! The core returns a decision and a deadline. This crate owns the clock, the
//! sockets and the tasks that act on them — device discovery and binding, the
//! poll scheduler, the COV subscription lifecycle with its renewal before
//! lapse, reconnection with backoff, and the event sink a notification
//! recipient needs.
//!
//! # Lifecycle is explicit
//!
//! `new` constructs and starts nothing, so a caller can build, inspect and
//! discard. `run` blocks until cancelled or a fatal error occurs, and does not
//! return while a task it spawned is still alive. `close` is idempotent and
//! safe after a failed `run`. A device going offline is an expected event
//! reported through the event stream, never an error return — reserve those
//! for conditions the caller must fix.
//!
//! # Layer contract
//!
//! Owns: the clock, the waiting, the device registry, subscription state,
//! and backpressure policy.
//!
//! Must not: decode a PDU. If the runtime needs to look inside a frame to make
//! a scheduling decision, the core is missing a decision the state machine
//! should have returned.
//!
//! Allowed imports: every datalink, `regalium-bacnet-core`,
//! `regalium-bacnet-schema`, `regalium-bacnet-telemetry`.
//!
//! # Spans
//!
//! `bacnet.runtime.{read,discover}` now; `poll`, `subscribe` and
//! `reconnect` with the scheduler and COV lifecycle.
//!
//! # State
//!
//! Phase 8: [`Client`] over any [`Datalink`](regalium_bacnet_core::datalink::Datalink)
//! — discovery, binding through routers, `ReadProperty` one at a time or
//! many in flight, readings into the protobuf model, and offline as an
//! event. It blocks, as the datalinks do; the poll scheduler, COV lifecycle
//! and reconnection come next, and an async surface wraps this one rather
//! than replacing it.

mod alarm;
mod batch;
mod client;
mod config;
mod cov;
mod device;
mod discover;
mod error;
mod listen;
mod pump;
mod read;
mod reading;
mod rebind;
mod rpm;
mod serve;
mod server;
mod watch;
mod write;

pub use alarm::Alarm;
pub use client::Client;
pub use config::Config;
pub use device::{Device, Event, Route};
pub use error::{Error, RequestError};
pub use read::ReadRequest;
pub use reading::Reading;
pub use server::Server;
pub use watch::{Source, Update, Watch};
