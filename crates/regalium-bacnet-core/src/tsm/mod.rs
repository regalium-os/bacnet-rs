// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The transaction state machine (§5.4): what turns "send this request" into
//! the datagrams, retries, segments and acknowledgements that get an answer
//! back — or the decision that none is coming.
//!
//! # Sans-io
//!
//! [`Tsm`] is handed a request, each APDU that arrives and the current time,
//! and answers with [`Step`]s: transmit these octets, this transaction is
//! complete, this one is abandoned, nothing to do until this deadline. It
//! never sends, never sleeps and never reads a clock. Time is a
//! [`Duration`](core::time::Duration) since whatever origin the runtime
//! chose, and only differences between two of them mean anything.
//!
//! That is the whole point: a retry storm, a segmented answer that stops
//! half way, a device that goes quiet mid-transaction — each is a table of
//! events and expected steps on a fake clock, replayed identically every
//! run. The corpus under `fixtures/tsm/` is that table.
//!
//! # Scope
//!
//! The client side of §5.4.4: invoke ids, the APDU timeout and retries,
//! segmenting a request a peer cannot take whole, reassembling a segmented
//! answer, and aborting what makes no sense. The server side (§5.4.5),
//! which answers requests made of a local device, arrives with the object
//! model that has something to answer with.
//!
//! # Spans
//!
//! `bacnet.tsm.transaction`, one per request from open to end, recording
//! the invoke id, service choice, whether it was segmented, attempts and
//! outcome — never the request or the answer.

mod config;
mod machine;
mod open;
mod reassemble;
mod receive;
mod step;
mod txn;

pub use config::{Config, Remote, Request};
pub use machine::Tsm;
pub use step::{Abandoned, Received, Refused, Reply, Step};
