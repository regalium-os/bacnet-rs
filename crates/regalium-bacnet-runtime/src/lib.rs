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
//! Owns: the clock, tokio tasks, sockets, the device registry, subscription
//! state, and backpressure policy.
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
//! `bacnet.runtime.{poll,discover,subscribe,reconnect}`.
