// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! A simulated `BACnet`/IP lift gateway, so the probe can be exercised — and
//! its output understood — with no hardware.
//!
//! It answers Who-Is, Who-Has, `ReadProperty` and `ReadPropertyMultiple`;
//! accepts `WriteProperty`, `WritePropertyMultiple`, `CreateObject` and
//! `DeleteObject`, refusing them with a controller's error codes; keeps COV
//! subscriptions and notifies changes, including written ones; and accepts
//! foreign device registration as a BBMD would. Each capability can be switched off
//! to imitate an older controller, which is how the probe's fallbacks are
//! tested.

mod answer;
mod change;
mod cov;
mod data;
mod write;

use crate::frame::{self, Delivery, bvll};
use crate::tag::{ObjectId, Value};
use std::collections::HashSet;
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

/// The gateway's device object.
pub const DEVICE: ObjectId = ObjectId::new(8, 599);
/// Car 1 load cell, kilograms.
pub const CAR_LOAD: ObjectId = ObjectId::new(0, 1);
/// Machine room temperature, in alarm.
pub const MACHINE_ROOM: ObjectId = ObjectId::new(0, 2);
/// Machine room fan, commanded at priority 8.
pub const FAN: ObjectId = ObjectId::new(1, 1);
/// Car 1 door.
pub const DOOR: ObjectId = ObjectId::new(3, 1);
/// Car 1 operating mode.
pub const MODE: ObjectId = ObjectId::new(19, 1);

/// The simulator. Single-threaded: [`Sim::serve`] owns it.
#[allow(clippy::struct_excessive_bools)] // Independent capability switches.
pub struct Sim {
    props: data::Table,
    live: bool,
    started: Instant,
    rpm: bool,
    cov: bool,
    accept_foreign: bool,
    subs: Vec<cov::Sub>,
    /// Objects whose present-value was written, or that were created: they
    /// hold what was written instead of moving when live.
    pinned: HashSet<ObjectId>,
    /// Objects created at runtime, and so the only ones that may be deleted.
    created: HashSet<ObjectId>,
}

impl Sim {
    /// A gateway with one car: load, door, mode, a fan, and a hot machine
    /// room. Values are fixed until [`Sim::live`].
    #[must_use]
    pub fn lift_gateway() -> Self {
        Self {
            props: data::lift_gateway(),
            live: false,
            started: Instant::now(),
            rpm: true,
            cov: true,
            accept_foreign: true,
            subs: Vec::new(),
            pinned: HashSet::new(),
            created: HashSet::new(),
        }
    }

    /// Makes present-values move over time, so watching has something to show.
    #[must_use]
    pub fn live(mut self) -> Self {
        self.live = true;
        self
    }

    /// Rejects `ReadPropertyMultiple` as unrecognized, like an older controller.
    #[must_use]
    pub fn without_rpm(mut self) -> Self {
        self.rpm = false;
        self
    }

    /// Rejects `SubscribeCOV` as unrecognized, so a watcher must poll.
    #[must_use]
    pub fn without_cov(mut self) -> Self {
        self.cov = false;
        self
    }

    /// NAKs foreign device registration, as a BBMD with a full table does.
    #[must_use]
    pub fn refusing_foreign_devices(mut self) -> Self {
        self.accept_foreign = false;
        self
    }

    /// Serves on `socket` until it errors. Blocks; checks COV subscriptions
    /// for changes every 100 ms.
    ///
    /// # Errors
    ///
    /// When the socket fails.
    pub fn serve(mut self, socket: &UdpSocket) -> io::Result<()> {
        socket.set_read_timeout(Some(Duration::from_millis(100)))?;
        let mut buf = [0u8; 1500];
        loop {
            match socket.recv_from(&mut buf) {
                Ok((n, from)) => self.handle(socket, buf.get(..n).unwrap_or_default(), from)?,
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) => {}
                Err(e) => return Err(e),
            }
            self.tick(socket)?;
        }
    }

    /// Binds `127.0.0.1:0`, serves on a background thread, returns the
    /// address. The thread runs until the process ends.
    ///
    /// # Errors
    ///
    /// When the socket cannot be bound.
    pub fn spawn(self) -> io::Result<SocketAddr> {
        let socket = UdpSocket::bind("127.0.0.1:0")?;
        let addr = socket.local_addr()?;
        std::thread::spawn(move || self.serve(&socket));
        Ok(addr)
    }

    fn handle(&mut self, socket: &UdpSocket, datagram: &[u8], from: SocketAddr) -> io::Result<()> {
        if bvll::registration(datagram).is_some() {
            let code = if self.accept_foreign {
                0
            } else {
                bvll::REGISTER_NAK
            };
            socket.send_to(&bvll::result(code), from)?;
            return Ok(());
        }
        let Some(msg) = frame::decode(datagram, from) else {
            return Ok(());
        };
        for reply in self.answer(&msg) {
            socket.send_to(
                &frame::encode(Delivery::Unicast, None, false, &reply),
                msg.from,
            )?;
        }
        Ok(())
    }

    /// The present-value of `object` now: fixed, or moving when live.
    fn present(&self, object: ObjectId) -> Option<Value> {
        let fixed = self.props.get(&(object, 85))?.first()?.clone();
        if !self.live || self.pinned.contains(&object) {
            return Some(fixed);
        }
        let step = self.started.elapsed().as_millis() / 700;
        let pick = |n: u128| usize::try_from(step % n).unwrap_or(0);
        Some(match object {
            CAR_LOAD => Value::Real(
                [0.0, 75.0, 420.0, 610.0, 420.0, 150.0]
                    .get(pick(6))
                    .copied()
                    .unwrap_or(0.0),
            ),
            MACHINE_ROOM => Value::Real(
                [41.5, 41.75, 42.0, 42.25, 42.5]
                    .get(pick(5))
                    .copied()
                    .unwrap_or(41.5),
            ),
            DOOR => Value::Enumerated(u32::from(pick(3) == 0)),
            MODE => Value::Unsigned(if pick(9) == 8 { 3 } else { 2 }),
            _ => fixed,
        })
    }
}
