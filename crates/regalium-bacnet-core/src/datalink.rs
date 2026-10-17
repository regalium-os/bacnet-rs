// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The boundary every datalink satisfies: move one NPDU to a MAC address, or
//! wait a bounded time for one to arrive.
//!
//! The core declares it because the core is the consumer: the runtime drives
//! the transaction state machine, which says what to send and how long to
//! wait, and a datalink is anything that can do those two things. BACnet/IP,
//! MS/TP, BACnet/SC and BACnet/IPv6 each implement it in their own crate,
//! and a proprietary bridge implements it outside this repository without
//! this one being involved.
//!
//! # Why blocking with a timeout
//!
//! [`Datalink::receive`] takes the time left until the state machine's next
//! deadline — exactly what [`Step::Idle`](crate::tsm::Step::Idle) returns —
//! and gives up when it passes. That needs no executor, so a datalink is
//! plain `std`; a runtime that wants async wraps it, rather than every
//! datalink choosing a runtime for its callers.

use core::fmt::Debug;
use core::time::Duration;

/// Where a datalink sends an NPDU.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Recipient<M> {
    /// One station on the local network.
    Unicast(M),
    /// Every station on the local network. A broadcast to a remote or every
    /// network is still a local broadcast here; the NPDU's destination says
    /// how far routers carry it (§6.3).
    LocalBroadcast,
}

/// An NPDU that arrived: who sent it, and how many octets of the caller's
/// buffer it fills.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Received<M> {
    /// The sender's MAC. For a message a BBMD forwarded, the station that
    /// originally sent it — the one an answer goes to (Annex J.4.5).
    pub from: M,
    /// Octets of NPDU written to the buffer.
    pub len: usize,
    /// Whether it was sent to every station rather than to this one.
    pub broadcast: bool,
}

/// A datalink: one network's way of carrying NPDUs.
///
/// # Concurrency
///
/// Methods take `&self` so one task can receive while another sends; an
/// implementation says in its own docs whether that is safe for it.
pub trait Datalink {
    /// A station's address on this network: an IP address and port, an
    /// MS/TP node number, a virtual MAC. Ordered and hashable so it can key
    /// a transaction.
    type Mac: Copy + Ord + core::hash::Hash + Debug + Send;

    /// What the caller must fix when the datalink cannot work — a closed
    /// socket, a port that went away. Never a quiet peer; that is a timeout.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Sends one NPDU.
    ///
    /// # Errors
    ///
    /// When the datalink itself failed.
    fn send(&self, to: Recipient<Self::Mac>, npdu: &[u8]) -> Result<(), Self::Error>;

    /// Waits at most `timeout` for the next NPDU, copying it into `buf`.
    /// `Ok(None)` when the time passed with nothing to deliver. Datalink
    /// housekeeping — a BVLC-Result, a registration due — happens inside
    /// the wait and is never returned.
    ///
    /// # Errors
    ///
    /// When the datalink itself failed.
    fn receive(
        &self,
        buf: &mut [u8],
        timeout: Duration,
    ) -> Result<Option<Received<Self::Mac>>, Self::Error>;

    /// The largest APDU this datalink carries, for the
    /// [`Remote`](crate::tsm::Remote) limits of every peer on it.
    fn max_apdu(&self) -> u16;
}
