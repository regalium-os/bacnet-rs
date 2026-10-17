// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! A blocking `BACnet`/IP client over one UDP socket: it reads, writes,
//! creates and deletes.
//!
//! One socket carries everything: replies to our requests, I-Am and I-Have
//! answers, COV notifications, BVLC results. Whatever arrives while waiting
//! for one thing and belongs to another is kept, not dropped — a COV
//! notification that lands mid-read is queued for [`Client::notifications`].
//!
//! # Concurrency
//!
//! `Send`, so it can live on a worker thread, but not shareable: every
//! method takes `&mut self` because replies are matched on a single socket,
//! and two threads waiting on it would steal each other's replies.

mod cov;
mod discovery;
mod error;
mod foreign;
mod found;
mod observe;
mod plan;
mod read;
mod socket;
mod target;
mod transact;
mod write;

pub use error::ReadError;
pub use foreign::ForeignError;
pub use plan::Discovery;

use crate::frame::Route;
use crate::service::cov::Notification;
use regalium_bacnet_telemetry::{Logger, Meter, NoopLogger, NoopMeter, NoopTracer, SpanId, Tracer};
use std::collections::{HashMap, HashSet, VecDeque};
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::Duration;

/// A device that answered Who-Is with I-Am (§16.10), or I-Have.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    /// Device instance, unique on the internetwork.
    pub instance: u32,
    /// The IP endpoint to send to: the device itself, or its router.
    pub addr: SocketAddr,
    /// Set when the device is behind a router (e.g. MS/TP behind a gateway).
    pub route: Option<Route>,
    /// Largest APDU it accepts; 480 when it was found by I-Have and not asked.
    pub max_apdu: u64,
    /// ASHRAE vendor identifier; 0 when unknown.
    pub vendor: u64,
}

/// Which way a traced datagram went.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Leaving this host.
    Sent,
    /// Arriving.
    Received,
}

impl Direction {
    /// The `bacnet.direction` label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Sent => "sent",
            Self::Received => "received",
        }
    }
}

/// Called with every datagram the client sends or receives, before decoding.
pub type Trace = Box<dyn Fn(Direction, SocketAddr, &[u8]) + Send>;

/// The client. See the module documentation for its concurrency rules.
pub struct Client {
    socket: UdpSocket,
    invoke: u8,
    timeout: Duration,
    retries: u32,
    trace: Option<Trace>,
    meter: Arc<dyn Meter>,
    tracer: Arc<dyn Tracer>,
    logger: Arc<dyn Logger>,
    /// Open spans, innermost last: what a request is made inside.
    stack: Vec<SpanId>,
    /// What each device has said about itself, labelling its requests.
    identities: HashMap<u32, crate::Identity>,
    /// The device the latest request went to: who a log line written
    /// after it, in the same call or step, is about.
    last_device: Option<u32>,
    pending: VecDeque<Notification>,
    foreign: Option<foreign::Registration>,
    next_process: u32,
    no_rpm: HashSet<u32>,
}

impl Client {
    /// Binds `addr` with broadcast enabled. Defaults: 2 s timeout, 2 retries,
    /// no trace, a no-op meter, tracer and logger. Spawns nothing.
    ///
    /// # Errors
    ///
    /// When the socket cannot be bound, e.g. the port is taken.
    pub fn bind(addr: SocketAddr) -> io::Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_broadcast(true)?;
        Ok(Self {
            socket,
            invoke: 0,
            timeout: Duration::from_secs(2),
            retries: 2,
            trace: None,
            meter: Arc::new(NoopMeter),
            tracer: Arc::new(NoopTracer),
            logger: Arc::new(NoopLogger),
            stack: Vec::new(),
            identities: HashMap::new(),
            last_device: None,
            pending: VecDeque::new(),
            foreign: None,
            next_process: 1,
            no_rpm: HashSet::new(),
        })
    }

    /// How long to wait for each reply before retrying.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// How many times to resend an unanswered request.
    #[must_use]
    pub fn with_retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }

    /// Observes every datagram on the wire, e.g. to dissect it.
    #[must_use]
    pub fn with_trace(
        mut self,
        trace: impl Fn(Direction, SocketAddr, &[u8]) + Send + 'static,
    ) -> Self {
        self.trace = Some(Box::new(trace));
        self
    }

    /// Records request, packet and byte metrics through `meter`.
    #[must_use]
    pub fn with_meter(mut self, meter: Arc<dyn Meter>) -> Self {
        self.meter = meter;
        self
    }

    /// The meter this client records through, for callers recording point
    /// metrics alongside its request metrics.
    #[must_use]
    pub fn meter(&self) -> &dyn Meter {
        self.meter.as_ref()
    }

    /// Whether device `instance` is worth asking for `ReadPropertyMultiple`:
    /// true until it rejects the service once.
    #[must_use]
    pub fn supports_rpm(&self, instance: u32) -> bool {
        !self.no_rpm.contains(&instance)
    }

    /// The bound address.
    ///
    /// # Errors
    ///
    /// When the operating system cannot report it.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }
}
