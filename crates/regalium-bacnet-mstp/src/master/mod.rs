// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The master node state machine (§9.5.6), on a clock the caller supplies.
//!
//! An RS-485 segment has one speaker at a time, and the token says who.
//! Each master that holds it sends up to `max_info_frames` frames, then
//! passes it on; every `Npoll` passes it instead looks for a master between
//! itself and its successor, which is how a controller added to a running
//! bus is found. Silence longer than `Tno_token` means the token was lost,
//! and the master whose slot comes first after the silence makes a new one.
//!
//! None of it waits. [`Master::received`] and [`Master::poll`] return what
//! to send and what to hand up, and [`Master::deadline`] when to call
//! [`Master::poll`] next; the link owns the line and the waiting.

mod config;
mod receive;
mod token;

pub use config::Config;

use crate::frame::{Frame, kind};
use core::time::Duration;
use std::collections::VecDeque;

/// What the machine asks of its link.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    /// Put this frame on the line.
    Send(Frame),
    /// An NPDU for this station from `source`.
    Deliver {
        /// The sending station.
        source: u8,
        /// The NPDU.
        npdu: Vec<u8>,
        /// Sent to every station.
        broadcast: bool,
    },
}

/// Where the machine is (§9.5.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum State {
    /// Listening; another master holds the token.
    Idle,
    /// The line has been silent too long: the token is presumed lost.
    NoToken,
    /// Waiting for the answer to a request this station sent.
    WaitForReply,
    /// Passed the token; waiting to hear the successor use it.
    PassToken,
    /// Asking a station whether it is a master.
    PollForMaster,
    /// A request arrived; waiting for the local answer to it.
    AnswerDataRequest,
    /// No other master answers: this one keeps the token.
    SoleMaster,
}

/// One frame waiting for the token.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Outgoing {
    pub(crate) destination: u8,
    pub(crate) npdu: Vec<u8>,
    pub(crate) expecting_reply: bool,
}

/// A master node.
///
/// Plain data: `Send` and `Sync`. One per station; the link that owns it
/// feeds it every frame heard and calls [`Master::poll`] at its deadline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Master {
    pub(crate) config: Config,
    pub(crate) state: State,
    pub(crate) next_station: u8,
    pub(crate) poll_station: u8,
    pub(crate) token_count: u8,
    pub(crate) frame_count: u8,
    pub(crate) retry_count: u8,
    /// The station a pending request came from, while answering it.
    pub(crate) answering: u8,
    pub(crate) deadline: Duration,
    pub(crate) queue: VecDeque<Outgoing>,
}

impl Master {
    /// A master starting at `now`: listening, and presuming the token lost
    /// if nothing is heard within `Tno_token`.
    #[must_use]
    pub const fn new(config: Config, now: Duration) -> Self {
        Self {
            config,
            state: State::Idle,
            next_station: config.station,
            poll_station: config.station,
            // As if a poll were due, so the first token held finds a successor.
            token_count: token::NPOLL,
            frame_count: 0,
            retry_count: 0,
            answering: 0,
            deadline: now.saturating_add(config.no_token),
            queue: VecDeque::new(),
        }
    }

    /// The state, for a status display.
    #[must_use]
    pub const fn state(&self) -> State {
        self.state
    }

    /// The station this master passes the token to.
    #[must_use]
    pub const fn next_station(&self) -> u8 {
        self.next_station
    }

    /// When [`Master::poll`] next has something to do.
    #[must_use]
    pub const fn deadline(&self) -> Duration {
        self.deadline
    }

    /// Queues an NPDU for `destination` ([`BROADCAST`](crate::frame::BROADCAST) for every station),
    /// sent when this master next holds the token — or at once, when it is
    /// the answer to a request it is waiting to answer.
    pub fn enqueue(
        &mut self,
        destination: u8,
        npdu: Vec<u8>,
        expecting_reply: bool,
        now: Duration,
    ) -> Vec<Action> {
        let out = Outgoing {
            destination,
            npdu,
            expecting_reply,
        };
        if self.state == State::AnswerDataRequest
            && destination == self.answering
            && !expecting_reply
        {
            self.state = State::Idle;
            self.deadline = now.saturating_add(self.config.no_token);
            return vec![Action::Send(data_frame(&out, self.config.station))];
        }
        self.queue.push_back(out);
        if self.state == State::SoleMaster {
            return self.use_token(now);
        }
        Vec::new()
    }
}

fn deliver(frame: &Frame, broadcast: bool) -> Vec<Action> {
    let data = matches!(
        frame.kind,
        kind::DATA_EXPECTING_REPLY
            | kind::DATA_NOT_EXPECTING_REPLY
            | kind::EXTENDED_EXPECTING_REPLY
            | kind::EXTENDED_NOT_EXPECTING_REPLY
    );
    if !data {
        return Vec::new();
    }
    vec![Action::Deliver {
        source: frame.source,
        npdu: frame.data.clone(),
        broadcast,
    }]
}

/// The data frame for `out`: extended when the NPDU is too large for an
/// ordinary one (Annex T).
fn data_frame(out: &Outgoing, me: u8) -> Frame {
    let big = out.npdu.len() > crate::frame::MAX_DATA;
    let kind = match (out.expecting_reply, big) {
        (true, false) => kind::DATA_EXPECTING_REPLY,
        (false, false) => kind::DATA_NOT_EXPECTING_REPLY,
        (true, true) => kind::EXTENDED_EXPECTING_REPLY,
        (false, true) => kind::EXTENDED_NOT_EXPECTING_REPLY,
    };
    Frame {
        kind,
        destination: out.destination,
        source: me,
        data: out.npdu.clone(),
    }
}
