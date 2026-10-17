// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! [`BacnetIp`]: the UDP socket, and the BVLL around every NPDU it carries.

mod receive;

use crate::bbmd::{Bbmd, Out, Target};
use crate::bvll::{Bvll, Function};
use crate::foreign::Registration;
use regalium_bacnet_core::datalink::{Datalink, Recipient};
use regalium_bacnet_telemetry::{Attribute, NoopTracer, Span, Tracer, keys};
use std::io;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

/// The largest APDU BACnet/IP carries: what fits a 1500-octet Ethernet frame
/// after IP, UDP, BVLL and NPDU headers (Annex J.1).
pub const MAX_APDU: u16 = 1476;

/// How this node takes part in broadcast distribution.
#[derive(Debug)]
enum Mode {
    /// On a subnet whose broadcasts are enough, or with a BBMD of its own.
    Plain,
    /// Registered with a BBMD on another subnet.
    Foreign(Registration),
    /// The BBMD for its subnet.
    Bbmd(Bbmd),
}

/// A BACnet/IP datalink (Annex J) on one UDP socket.
///
/// [`BacnetIp::bind`] opens the socket and does nothing else: no thread, no
/// datagram. Registration with a BBMD happens inside [`Datalink::receive`],
/// so it renews for as long as something is receiving.
///
/// # Concurrency
///
/// `Send` and `Sync`. One thread may send while another receives; two
/// receivers would split the datagrams between them, which is never what a
/// caller wants.
pub struct BacnetIp {
    socket: UdpSocket,
    local: SocketAddrV4,
    broadcast: SocketAddrV4,
    mode: Mutex<Mode>,
    origin: Instant,
    tracer: Arc<dyn Tracer>,
}

impl BacnetIp {
    /// Binds `addr` — `0.0.0.0:47808` for a device, port 0 for a client
    /// that only asks — with broadcasting allowed. Local broadcasts go to
    /// `255.255.255.255` on the bound port until told otherwise.
    ///
    /// # Errors
    ///
    /// When the socket cannot be bound.
    pub fn bind(addr: SocketAddrV4) -> io::Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_broadcast(true)?;
        let local = match socket.local_addr()? {
            std::net::SocketAddr::V4(a) => a,
            std::net::SocketAddr::V6(_) => return Err(io::ErrorKind::AddrNotAvailable.into()),
        };
        Ok(Self {
            socket,
            local,
            broadcast: SocketAddrV4::new(Ipv4Addr::BROADCAST, local.port()),
            mode: Mutex::new(Mode::Plain),
            origin: Instant::now(),
            tracer: Arc::new(NoopTracer),
        })
    }

    /// Where local broadcasts go: the subnet's directed broadcast address
    /// (`192.168.1.255:47808`), which routers do not pass.
    #[must_use]
    pub const fn broadcast_address(mut self, addr: SocketAddrV4) -> Self {
        self.broadcast = addr;
        self
    }

    /// Registers with `bbmd` as a foreign device for `ttl` seconds (Annex
    /// J.5.2), sending broadcasts through it from then on.
    #[must_use]
    pub fn foreign(self, bbmd: SocketAddrV4, ttl: u16) -> Self {
        self.with_mode(Mode::Foreign(Registration::new(bbmd, ttl)))
    }

    /// Acts as this subnet's BBMD.
    #[must_use]
    pub fn bbmd(self, bbmd: Bbmd) -> Self {
        self.with_mode(Mode::Bbmd(bbmd))
    }

    fn with_mode(self, mode: Mode) -> Self {
        *self.mode.lock().unwrap_or_else(PoisonError::into_inner) = mode;
        self
    }

    /// Opens `bacnet.ip.*` spans through `tracer`.
    #[must_use]
    pub fn with_tracer(mut self, tracer: Arc<dyn Tracer>) -> Self {
        self.tracer = tracer;
        self
    }

    /// The bound address.
    #[must_use]
    pub const fn local_addr(&self) -> SocketAddrV4 {
        self.local
    }

    /// Whether a foreign registration is in force; `false` when not foreign.
    #[must_use]
    pub fn is_registered(&self) -> bool {
        let now = self.now();
        matches!(&*self.lock(), Mode::Foreign(r) if r.is_registered(now))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Mode> {
        self.mode.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Time since this link was bound: the clock its state machines run on.
    fn now(&self) -> Duration {
        self.origin.elapsed()
    }

    /// Sends one BVLL message inside a `bacnet.ip.send` span.
    fn transmit(&self, bvll: &[u8], to: SocketAddrV4) -> io::Result<()> {
        let span = Span::start(&*self.tracer, "bacnet.ip.send");
        if let Ok(b) = Bvll::decode(bvll) {
            span.set(Attribute::str(keys::BVLC_FUNCTION, b.name()));
        }
        span.set(Attribute::int(
            keys::PDU_OCTETS,
            i64::try_from(bvll.len()).unwrap_or(i64::MAX),
        ));
        match self.socket.send_to(bvll, to) {
            Ok(_) => {
                span.end();
                Ok(())
            }
            Err(e) => {
                span.fail(&e);
                Err(e)
            }
        }
    }

    fn deliver(&self, out: &[Out]) -> io::Result<()> {
        for o in out {
            let to = match o.to {
                Target::Unicast(a) => a,
                Target::LocalBroadcast => self.broadcast,
            };
            self.transmit(&o.bvll, to)?;
        }
        Ok(())
    }
}

impl Datalink for BacnetIp {
    type Mac = SocketAddrV4;
    type Error = io::Error;

    fn send(&self, to: Recipient<SocketAddrV4>, npdu: &[u8]) -> io::Result<()> {
        let mode = self.lock();
        match (to, &*mode) {
            (Recipient::Unicast(a), _) => {
                self.transmit(&Bvll::to_vec(Function::OriginalUnicast(npdu)), a)
            }
            (Recipient::LocalBroadcast, Mode::Plain) => self.transmit(
                &Bvll::to_vec(Function::OriginalBroadcast(npdu)),
                self.broadcast,
            ),
            (Recipient::LocalBroadcast, Mode::Foreign(r)) => {
                self.transmit(&Bvll::to_vec(Function::DistributeBroadcast(npdu)), r.bbmd())
            }
            (Recipient::LocalBroadcast, Mode::Bbmd(b)) => self.deliver(&b.broadcast(npdu)),
        }
    }

    fn receive(
        &self,
        buf: &mut [u8],
        timeout: Duration,
    ) -> io::Result<Option<regalium_bacnet_core::datalink::Received<SocketAddrV4>>> {
        self.receive_until(buf, Instant::now() + timeout)
    }

    fn max_apdu(&self) -> u16 {
        MAX_APDU
    }
}
