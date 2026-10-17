// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! [`Mstp`]: the master node on a [`Line`], driven by the calls a
//! [`Datalink`] receives.

mod drive;
mod serial;

use crate::frame::BROADCAST;
use crate::master::{Config, Master, State};
use crate::reader::Reader;
use regalium_bacnet_core::datalink::{Datalink, Received, Recipient};
use regalium_bacnet_telemetry::{NoopTracer, Tracer};
use std::collections::VecDeque;
use std::io;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// The NPDU control octet's data-expecting-reply bit (§6.2.2).
const EXPECTING_REPLY: u8 = 0x04;

/// The octets an MS/TP link sends and hears: an RS-485 port, or anything
/// that behaves like one.
///
/// Declared here, where it is used, so a test bus or a vendor's USB adapter
/// can stand in for a serial port.
pub trait Line: Send {
    /// Reads what has arrived, waiting at most `timeout`; `Ok(0)` when
    /// nothing did.
    ///
    /// # Errors
    ///
    /// When the port failed, not when it was quiet.
    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize>;

    /// Puts `octets` on the line.
    ///
    /// # Errors
    ///
    /// When the port failed.
    fn write(&mut self, octets: &[u8]) -> io::Result<()>;
}

/// What the link keeps between calls.
#[derive(Debug)]
struct Inner {
    master: Master,
    reader: Reader,
    /// NPDUs for this station, received but not yet handed up.
    inbox: VecDeque<(u8, Vec<u8>, bool)>,
}

/// An MS/TP master datalink (Clause 9) on one line.
///
/// MS/TP has no background: the token moves only while someone calls
/// [`Datalink::receive`], which is where frames are read, answered and
/// passed on. A `Client` or `Server` does so continuously; anything else
/// using this link directly must too, or the bus stalls at this station.
///
/// # Concurrency
///
/// `Send` and `Sync`. A send waits while a receive is reading, for at most
/// the master's next deadline.
pub struct Mstp<L> {
    line: Mutex<L>,
    inner: Mutex<Inner>,
    origin: Instant,
    tracer: Arc<dyn Tracer>,
}

impl<L: Line> Mstp<L> {
    /// A master on `line`, listening; it takes part in the token from the
    /// first [`Datalink::receive`].
    #[must_use]
    pub fn new(line: L, config: Config) -> Self {
        Self {
            line: Mutex::new(line),
            inner: Mutex::new(Inner {
                master: Master::new(config, Duration::ZERO),
                reader: Reader::new(),
                inbox: VecDeque::new(),
            }),
            origin: Instant::now(),
            tracer: Arc::new(NoopTracer),
        }
    }

    /// Opens `bacnet.mstp.*` spans through `tracer`.
    #[must_use]
    pub fn with_tracer(mut self, tracer: Arc<dyn Tracer>) -> Self {
        self.tracer = tracer;
        self
    }

    /// Where the master is, for a status display.
    pub fn state(&self) -> State {
        self.lock().master.state()
    }

    /// How many frames were thrown away for a bad check: noise on the line.
    pub fn discarded(&self) -> usize {
        self.lock().reader.discarded()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn now(&self) -> Duration {
        self.origin.elapsed()
    }
}

impl<L: Line> Datalink for Mstp<L> {
    type Mac = u8;
    type Error = io::Error;

    fn send(&self, to: Recipient<u8>, npdu: &[u8]) -> io::Result<()> {
        let (destination, broadcast) = match to {
            Recipient::Unicast(m) => (m, false),
            Recipient::LocalBroadcast => (BROADCAST, true),
        };
        let expecting = !broadcast && npdu.get(1).is_some_and(|c| c & EXPECTING_REPLY != 0);
        let now = self.now();
        let actions = self
            .lock()
            .master
            .enqueue(destination, npdu.to_vec(), expecting, now);
        self.act(actions)
    }

    fn receive(&self, buf: &mut [u8], timeout: Duration) -> io::Result<Option<Received<u8>>> {
        self.receive_until(buf, Instant::now() + timeout)
    }

    /// The largest APDU an ordinary frame carries (Table 6-1); extended
    /// frames carry more, but a peer that predates Annex T cannot hear them.
    fn max_apdu(&self) -> u16 {
        480
    }
}
