// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Holding, passing and recovering the token (§9.5.6.4–9.5.6.8), and what
//! each state does when its deadline passes.

use super::{Action, Master, State, data_frame};
use crate::frame::{BROADCAST, Frame, kind};
use core::time::Duration;

/// Token passes between polls for a new master (`Npoll`, §9.5.3).
pub(super) const NPOLL: u8 = 50;
/// Times a token is passed again when the successor does not take it.
const NRETRY_TOKEN: u8 = 1;

impl Master {
    /// What is due at `now`: a reply that never came, a successor that
    /// never took the token, a silence long enough to mean it was lost.
    /// Nothing before [`Master::deadline`].
    pub fn poll(&mut self, now: Duration) -> Vec<Action> {
        if now < self.deadline {
            return Vec::new();
        }
        let me = self.config.station;
        match self.state {
            State::Idle => {
                // Lost: the lowest station wakes first and makes a new one.
                self.state = State::NoToken;
                self.deadline = now.saturating_add(self.config.slot.saturating_mul(u32::from(me)));
                Vec::new()
            }
            State::NoToken => {
                self.next_station = me;
                self.poll_station = self.after(me);
                self.poll_for_master(now)
            }
            State::PassToken if self.retry_count < NRETRY_TOKEN => {
                self.retry_count += 1;
                self.send_token(now)
            }
            State::PassToken => {
                // The successor is gone: look for the next one after it.
                self.poll_station = self.after(self.next_station);
                self.next_station = me;
                self.poll_for_master(now)
            }
            State::PollForMaster if self.next_station != me => self.pass_token(now),
            State::PollForMaster => {
                self.poll_station = self.after(self.poll_station);
                if self.poll_station == me {
                    // The whole bus was just polled: no need to poll it again.
                    self.state = State::SoleMaster;
                    self.frame_count = 0;
                    self.token_count = 0;
                    return self.use_token(now);
                }
                self.poll_for_master(now)
            }
            State::WaitForReply => {
                self.frame_count = self.frame_count.saturating_add(1);
                self.done_with_token(now)
            }
            State::AnswerDataRequest => {
                // Not answered in time: tell the requester to ask later.
                self.state = State::Idle;
                self.deadline = now.saturating_add(self.config.no_token);
                vec![Action::Send(Frame::bare(
                    kind::REPLY_POSTPONED,
                    self.answering,
                    me,
                ))]
            }
            State::SoleMaster => {
                self.frame_count = 0;
                self.use_token(now)
            }
        }
    }

    /// Holding the token: send what is queued, up to `max_info_frames`.
    pub(super) fn use_token(&mut self, now: Duration) -> Vec<Action> {
        let mut out = Vec::new();
        while self.frame_count < self.config.max_info_frames {
            let Some(next) = self.queue.pop_front() else {
                break;
            };
            out.push(Action::Send(data_frame(&next, self.config.station)));
            if next.expecting_reply && next.destination != BROADCAST {
                self.state = State::WaitForReply;
                self.deadline = now.saturating_add(self.config.reply_timeout);
                return out;
            }
            self.frame_count += 1;
        }
        out.extend(self.done_with_token(now));
        out
    }

    /// Done sending: pass the token on, or every `Npoll` passes look for a
    /// master between this one and its successor first.
    pub(super) fn done_with_token(&mut self, now: Duration) -> Vec<Action> {
        let me = self.config.station;
        self.token_count = self.token_count.saturating_add(1);
        if self.token_count < NPOLL {
            if self.next_station == me {
                self.state = State::SoleMaster;
                self.deadline = now.saturating_add(self.config.usage);
                return Vec::new();
            }
            return self.pass_token(now);
        }
        self.token_count = 0;
        self.poll_station = self.after(if self.next_station == me {
            me
        } else {
            self.poll_station
        });
        if self.poll_station == self.next_station && self.next_station != me {
            self.poll_station = me;
            return self.pass_token(now);
        }
        if self.poll_station == me {
            self.state = State::SoleMaster;
            self.deadline = now.saturating_add(self.config.usage);
            return Vec::new();
        }
        self.poll_for_master(now)
    }

    pub(super) fn pass_token(&mut self, now: Duration) -> Vec<Action> {
        self.retry_count = 0;
        self.send_token(now)
    }

    fn send_token(&mut self, now: Duration) -> Vec<Action> {
        self.state = State::PassToken;
        self.deadline = now.saturating_add(self.config.usage);
        vec![Action::Send(Frame::bare(
            kind::TOKEN,
            self.next_station,
            self.config.station,
        ))]
    }

    fn poll_for_master(&mut self, now: Duration) -> Vec<Action> {
        self.state = State::PollForMaster;
        self.deadline = now.saturating_add(self.config.usage);
        vec![Action::Send(Frame::bare(
            kind::POLL_FOR_MASTER,
            self.poll_station,
            self.config.station,
        ))]
    }

    /// The station after `s`, wrapping past `Nmax_master` to zero.
    const fn after(&self, s: u8) -> u8 {
        if s >= self.config.max_master {
            0
        } else {
            s + 1
        }
    }
}
