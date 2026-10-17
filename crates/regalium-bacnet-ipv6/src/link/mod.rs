// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! [`BacnetIpv6`]: the UDP socket, the multicast group, and the virtual MAC
//! table that lets an NPDU name a station in three octets.

mod receive;
mod send;

use crate::bvll::Vmac;
use crate::vmac_table::{Duplicate, VmacTable};
use regalium_bacnet_telemetry::{Attribute, NoopTracer, Span, Tracer, keys};
use std::collections::BTreeMap;
use std::io;
use std::net::{Ipv6Addr, SocketAddr, SocketAddrV6, UdpSocket};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// The link-local `BACnet` multicast group, `FF02::BAC0` (Annex U.4).
pub const LINK_LOCAL: Ipv6Addr = Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 0xbac0);
/// The site-local `BACnet` multicast group, `FF05::BAC0`.
pub const SITE_LOCAL: Ipv6Addr = Ipv6Addr::new(0xff05, 0, 0, 0, 0, 0, 0, 0xbac0);

/// Frames held per virtual MAC while it is resolved; beyond, the oldest go.
pub(super) const HELD: usize = 16;

/// What the link keeps between calls.
#[derive(Debug)]
pub(super) struct State {
    pub(super) table: VmacTable,
    /// NPDUs waiting for their destination to resolve.
    pub(super) held: BTreeMap<Vmac, Vec<Vec<u8>>>,
    pub(super) duplicates: Vec<Duplicate>,
}

/// A BACnet/IPv6 datalink (Annex U) on one UDP socket.
///
/// [`BacnetIpv6::bind`] opens the socket and does nothing else. Resolution
/// happens inside [`Datalink::receive`]: a unicast to an unknown virtual MAC
/// is held, Address-Resolution goes to the group, and the frame follows the
/// answer.
///
/// # Concurrency
///
/// `Send` and `Sync`. One thread may send while another receives.
pub struct BacnetIpv6 {
    pub(super) socket: UdpSocket,
    pub(super) group: SocketAddrV6,
    pub(super) state: Mutex<State>,
    pub(super) origin: Instant,
    pub(super) tracer: Arc<dyn Tracer>,
}

impl BacnetIpv6 {
    /// Binds `addr` as the station whose virtual MAC is `vmac`. Broadcasts
    /// go to the link-local group on the bound port until told otherwise;
    /// [`BacnetIpv6::join`] is what makes this socket hear them.
    ///
    /// # Errors
    ///
    /// When the socket cannot be bound.
    pub fn bind(addr: SocketAddrV6, vmac: Vmac) -> io::Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        let port = socket.local_addr()?.port();
        Ok(Self {
            socket,
            group: SocketAddrV6::new(LINK_LOCAL, port, 0, 0),
            state: Mutex::new(State {
                table: VmacTable::new(vmac),
                held: BTreeMap::new(),
                duplicates: Vec::new(),
            }),
            origin: Instant::now(),
            tracer: Arc::new(NoopTracer),
        })
    }

    /// Joins the group on interface `index` — the one this node is told to
    /// use, never a guess among several.
    ///
    /// # Errors
    ///
    /// When the operating system refuses the membership.
    pub fn join(self, index: u32) -> io::Result<Self> {
        self.socket.join_multicast_v6(self.group.ip(), index)?;
        Ok(self)
    }

    /// Where broadcasts and resolutions go: [`SITE_LOCAL`] on the right
    /// port, say, or a single address where multicast does not reach.
    #[must_use]
    pub const fn group(mut self, group: SocketAddrV6) -> Self {
        self.group = group;
        self
    }

    /// A virtual MAC known from configuration, never resolved or aged out.
    #[must_use]
    pub fn peer(self, vmac: Vmac, address: SocketAddrV6) -> Self {
        self.lock().table.insert(vmac, address);
        self
    }

    /// Opens `bacnet.ipv6.*` spans through `tracer`.
    #[must_use]
    pub fn with_tracer(mut self, tracer: Arc<dyn Tracer>) -> Self {
        self.tracer = tracer;
        self
    }

    /// The bound address.
    ///
    /// # Errors
    ///
    /// When the operating system cannot report it.
    pub fn local_addr(&self) -> io::Result<SocketAddrV6> {
        match self.socket.local_addr()? {
            SocketAddr::V6(a) => Ok(a),
            SocketAddr::V4(_) => Err(io::ErrorKind::AddrNotAvailable.into()),
        }
    }

    /// The duplicates seen since the last call: two addresses claiming one
    /// virtual MAC, which only someone with the network in front of them
    /// can settle.
    pub fn duplicates(&self) -> Vec<Duplicate> {
        std::mem::take(&mut self.lock().duplicates)
    }

    pub(super) fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(super) fn now(&self) -> Duration {
        self.origin.elapsed()
    }

    pub(super) fn own(&self) -> Vmac {
        self.lock().table.own()
    }

    /// Sends one BVLL-IPv6 message inside a `bacnet.ipv6.send` span.
    pub(super) fn transmit(&self, bvll: &[u8], to: SocketAddrV6) -> io::Result<()> {
        let span = Span::start(&*self.tracer, "bacnet.ipv6.send");
        span.set(Attribute::int(
            keys::PDU_OCTETS,
            i64::try_from(bvll.len()).unwrap_or(i64::MAX),
        ));
        match self.socket.send_to(bvll, to) {
            Ok(_) => Ok(()),
            Err(e) => {
                span.fail(&e);
                Err(e)
            }
        }
    }
}
