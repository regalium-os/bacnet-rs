// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The network thread. It owns the socket, so the UI never blocks on the
//! wire: the UI sends `Command`s, the worker streams `Event`s back.
//!
//! Long jobs (loading a device's objects, following live values) check for
//! new commands between round trips. A command that changes what the user
//! is looking at supersedes the job in flight, which is what keeps the UI
//! responsive on a slow MS/TP trunk where every read costs tens of
//! milliseconds.

mod change;
mod jobs;
mod live;
mod queue;
mod reopen;

use crate::args::Net;
use crate::connect::Connector;
use bacnet_probe::scan::ObjectReport;
use bacnet_probe::{Client, Device, Direction, Discovery, ObjectId, Value};
pub(crate) use change::Change;
use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

/// What the UI asks for.
#[derive(Debug)]
pub(crate) enum Command {
    /// Run the discovery plan; replies arrive as `Found`.
    Discover,
    /// Device summary, then its objects in batches.
    LoadDevice(Device),
    /// Every standard property of one object.
    LoadDetail(Device, ObjectId),
    /// Follow these present-values — by COV where allowed, else polled —
    /// replacing whatever was followed before; `None` stops.
    Watch(Option<(Device, Vec<ObjectId>)>),
    /// Poll interval for points without a subscription.
    SetInterval(Duration),
    /// Reopen the socket with these options, then rediscover; answered by
    /// `Connected` or `ConnectFailed`.
    Reconfigure(Net),
    /// Write, create or delete; answered by `Changed`.
    Change(Change),
}

/// What the worker reports.
#[derive(Debug)]
pub(crate) enum Event {
    Discovering,
    Found(Vec<Device>),
    DeviceInfo(u32, ObjectReport),
    ObjectCount(u32, u32),
    Object(u32, ObjectReport),
    /// The object list finished (`true`) or was superseded (`false`).
    DeviceDone(u32, bool),
    Detail(u32, ObjectReport),
    Value(u32, ObjectId, Result<Value, String>),
    /// How the followed points are fed: by COV, and by polling.
    Following {
        cov: usize,
        polled: usize,
    },
    Packet(Packet),
    Log(String),
    /// Reopened with the requested options, and discovering through them;
    /// what a person should know about how (its address, a registration).
    Connected(Vec<String>),
    /// The requested options could not be opened; the previous ones stand.
    ConnectFailed(String),
    /// A change was applied (`Ok`, with the object written or created) or
    /// refused; `what` says which, for a person.
    Changed {
        instance: u32,
        what: String,
        /// The object list changed (a create or delete), not just a value.
        structural: bool,
        result: Result<Option<ObjectId>, String>,
    },
}

/// One datagram, as the trace saw it.
#[derive(Debug, Clone)]
pub(crate) struct Packet {
    pub(crate) at: Instant,
    pub(crate) direction: Direction,
    pub(crate) peer: SocketAddr,
    pub(crate) bytes: Vec<u8>,
}

/// What the worker is busy with, deciding which new commands cut it short.
#[derive(Clone, Copy, PartialEq)]
enum Job {
    /// Loading this device's objects: only a Who-Is or another device
    /// supersedes it; detail requests are served in between batches.
    Device(u32),
    /// Loading one object's detail: any newer request supersedes it.
    Detail,
    /// Following live values: anything at all comes first.
    Live,
}

pub(crate) struct Worker {
    client: Client,
    plan: Discovery,
    commands: Receiver<Command>,
    events: Sender<Event>,
    pending: VecDeque<Command>,
    live: live::Live,
    /// Device names, for metrics recorded from here.
    names: HashMap<u32, String>,
    /// Each loaded point's name and units, so a live value is recorded on
    /// exactly the series its scan was.
    labels: HashMap<(u32, ObjectId), (String, Option<u32>)>,
    /// The options the socket was opened with, and how to open others;
    /// without it a `Reconfigure` is refused.
    reopen: Option<(Net, Connector)>,
    closed: bool,
}

impl Worker {
    /// Wires `client`'s trace into the event stream. Starts nothing.
    pub(crate) fn new(
        client: Client,
        plan: Discovery,
        commands: Receiver<Command>,
        events: Sender<Event>,
    ) -> Self {
        Self {
            client: reopen::traced(client, &events),
            plan,
            commands,
            events,
            pending: VecDeque::new(),
            live: live::Live::default(),
            names: HashMap::new(),
            labels: HashMap::new(),
            reopen: None,
            closed: false,
        }
    }

    /// Runs until the UI drops its command sender.
    pub(crate) fn run(mut self) {
        while !self.closed {
            if let Some(c) = self.pending.pop_front() {
                self.handle(c);
                continue;
            }
            if self.live.active() {
                self.live_step();
                self.drain();
                continue;
            }
            match self.commands.recv_timeout(Duration::from_secs(3600)) {
                Ok(c) => self.handle(c),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }

    fn handle(&mut self, command: Command) {
        match command {
            Command::Discover => self.discover(),
            Command::LoadDevice(device) => self.load_device(&device),
            // Only the newest detail request matters; skip stale ones.
            Command::LoadDetail(..)
                if self
                    .pending
                    .iter()
                    .any(|c| matches!(c, Command::LoadDetail(..))) => {}
            Command::LoadDetail(device, id) => self.load_detail(&device, id),
            Command::Watch(w) => self.live.want(w),
            Command::SetInterval(d) => self.live.interval = d,
            Command::Reconfigure(net) => self.reconfigure(net),
            Command::Change(c) => self.change(c),
        }
    }

    fn emit(&self, event: Event) {
        let _ = self.events.send(event);
    }
}

#[cfg(test)]
mod tests;
