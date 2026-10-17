// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Reopening the socket with new options, from the Settings tab.
//!
//! The order matters. Subscriptions are cancelled from the socket that made
//! them, since a device keeps notifying that port until told otherwise. The
//! old socket is closed before the new one is bound, because the new options
//! may well name the same port. And if the new options cannot be opened —
//! a port held by another process, a BBMD that refuses — the old ones are
//! opened again, so a typo costs the user an error message, not the session.

use super::{Command, Event, Packet, Worker};
use crate::args::Net;
use crate::connect::{Connector, Opened};
use bacnet_probe::Client;
use std::net::SocketAddr;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

/// How long to spend cancelling COV subscriptions before reopening. One the
/// budget does not reach lapses on the device within its lifetime (120 s).
const CANCEL_BUDGET: Duration = Duration::from_secs(2);

impl Worker {
    /// Lets `Reconfigure` reopen: `net` is what the client was opened with,
    /// `connect` opens any other options.
    #[must_use]
    pub(crate) fn with_connector(mut self, net: Net, connect: Connector) -> Self {
        self.reopen = Some((net, connect));
        self
    }

    pub(super) fn reconfigure(&mut self, net: Net) {
        self.drain();
        // A newer request supersedes this one before anything is torn down.
        if self
            .pending
            .iter()
            .any(|c| matches!(c, Command::Reconfigure(_)))
        {
            return;
        }
        let Some((previous, connect)) = self.reopen.take() else {
            return self.emit(Event::ConnectFailed(
                "this session cannot change its network settings".into(),
            ));
        };
        self.cancel_subscriptions();
        // Whatever was asked of the old network no longer applies.
        self.pending.retain(|c| {
            !matches!(
                c,
                Command::Discover | Command::LoadDevice(_) | Command::LoadDetail(..)
            )
        });
        // Release the port first: the new options may name the same one.
        let loopback = SocketAddr::from(([127, 0, 0, 1], 0));
        match Client::bind(loopback) {
            Ok(placeholder) => drop(std::mem::replace(&mut self.client, placeholder)),
            Err(e) => {
                self.reopen = Some((previous, connect));
                return self.emit(Event::ConnectFailed(format!("cannot reopen: {e}")));
            }
        }
        match connect(&net) {
            Ok(opened) => {
                let notes = self.adopt(opened);
                self.reopen = Some((net, connect));
                self.emit(Event::Connected(notes));
                self.discover();
            }
            Err(e) => {
                let why = match connect(&previous) {
                    Ok(opened) => {
                        self.adopt(opened);
                        format!("{e}; the previous settings are back in use")
                    }
                    Err(again) => format!("{e}; and the previous settings failed too: {again}"),
                };
                self.reopen = Some((previous, connect));
                self.emit(Event::ConnectFailed(why));
            }
        }
    }

    /// Cancels what is followed, from the socket that subscribed, within
    /// [`CANCEL_BUDGET`], and forgets everything the old network taught.
    fn cancel_subscriptions(&mut self) {
        self.live.want(None);
        let deadline = Instant::now() + CANCEL_BUDGET;
        for (device, id, process) in self.live.take_cancels() {
            if Instant::now() >= deadline {
                break;
            }
            let _ = self.client.cancel_cov(&device, id, process);
        }
        self.live.reset();
    }

    /// Makes `opened` the client and plan; returns its notes.
    fn adopt(&mut self, opened: Opened) -> Vec<String> {
        self.client = traced(opened.client, &self.events);
        self.plan = opened.plan;
        self.names.clear();
        self.labels.clear();
        opened.notes
    }
}

/// `client`, reporting every datagram as an `Event::Packet`.
pub(super) fn traced(client: Client, events: &Sender<Event>) -> Client {
    let packets = events.clone();
    client.with_trace(move |direction, peer, bytes| {
        let packet = Packet {
            at: Instant::now(),
            direction,
            peer,
            bytes: bytes.to_vec(),
        };
        let _ = packets.send(Event::Packet(packet));
    })
}
