// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! A local device: a database served on a datalink, answering what the
//! core's [`respond`] answers — Who-Is, Who-Has, reads and writes.

use crate::device::Route;
use regalium_bacnet_core::apdu::Apdu;
use regalium_bacnet_core::datalink::{Datalink, Recipient};
use regalium_bacnet_core::npdu::{Address, Body, Destination, GLOBAL, Npdu};
use regalium_bacnet_core::obj::Database;
use regalium_bacnet_core::server::{Responder, Response, respond};
use regalium_bacnet_core::service::unconfirmed;
use regalium_bacnet_core::tsm;
use regalium_bacnet_telemetry::{NoopTracer, Tracer};
use std::sync::Arc;
use std::time::Instant;

/// A local device serving `B` on `D`.
///
/// [`Server::new`] sends nothing; [`Server::serve`] answers until its time
/// is up, and is called again to go on. The database is the caller's to
/// read and change between calls — a gateway moving its points — through
/// [`Server::database_mut`].
///
/// # Concurrency
///
/// `Send` when the datalink and the database are. One thread serves; the
/// datalink may be shared with a client only if it says so.
pub struct Server<D: Datalink, B: Database> {
    pub(crate) link: D,
    pub(crate) db: B,
    pub(crate) tracer: Arc<dyn Tracer>,
    /// Large answers being sent in segments, by requester and its route.
    pub(crate) responder: Responder<(D::Mac, Option<Route>)>,
    /// The responder's clock.
    pub(crate) origin: Instant,
}

impl<D: Datalink, B: Database> Server<D, B> {
    /// A device serving `db` on `link`. Sends nothing.
    #[must_use]
    pub fn new(link: D, db: B) -> Self {
        Self {
            link,
            db,
            tracer: Arc::new(NoopTracer),
            responder: Responder::new(tsm::Config::default()),
            origin: Instant::now(),
        }
    }

    /// The segment timeout, retries and proposed window for answers sent in
    /// segments; [`tsm::Config::default`] otherwise.
    #[must_use]
    pub fn timers(mut self, config: tsm::Config) -> Self {
        self.responder = Responder::new(config);
        self
    }

    /// Opens a `bacnet.runtime.serve` span per request answered.
    #[must_use]
    pub fn with_tracer(mut self, tracer: Arc<dyn Tracer>) -> Self {
        self.tracer = tracer;
        self
    }

    /// The database.
    #[must_use]
    pub const fn database(&self) -> &B {
        &self.db
    }

    /// The database, to change between calls to [`Server::serve`].
    pub fn database_mut(&mut self) -> &mut B {
        &mut self.db
    }

    /// The datalink.
    #[must_use]
    pub const fn link(&self) -> &D {
        &self.link
    }

    /// Broadcasts I-Am, as a device does when it starts (§16.1), so clients
    /// already listening learn it without asking.
    ///
    /// # Errors
    ///
    /// The datalink's.
    pub fn announce(&mut self) -> Result<(), D::Error> {
        // The I-Am a Who-Is for everyone would get.
        let who_is = [0x10, unconfirmed::WHO_IS];
        let answer = Apdu::decode(&who_is)
            .ok()
            .and_then(|a| respond(&a, &mut self.db));
        match answer {
            Some(Response::Broadcast(i_am)) => self.broadcast(&i_am),
            _ => Ok(()),
        }
    }

    /// An unconfirmed APDU to every network: routers carry DNET X'FFFF'.
    pub(crate) fn broadcast(&self, apdu: &[u8]) -> Result<(), D::Error> {
        let npdu = Npdu {
            control: 0,
            destination: Some(Destination {
                address: Address {
                    network: GLOBAL,
                    mac: &[],
                },
                hop_count: 255,
            }),
            source: None,
            body: Body::Apdu(apdu),
        };
        let mut out = Vec::new();
        npdu.encode(&mut out);
        self.link.send(Recipient::LocalBroadcast, &out)
    }
}
