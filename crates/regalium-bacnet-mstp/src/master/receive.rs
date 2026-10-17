// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Every frame heard: the one this master waits for, or anything else,
//! which means another station has the line.

use super::{Action, Master, State, deliver};
use crate::frame::{BROADCAST, Frame, kind};
use core::time::Duration;

impl Master {
    /// A frame heard on the line at `now`, whoever it was for.
    pub fn received(&mut self, frame: &Frame, now: Duration) -> Vec<Action> {
        let me = self.config.station;
        let mine = frame.destination == me;
        match self.state {
            State::WaitForReply if mine => {
                self.frame_count = self.frame_count.saturating_add(1);
                let mut out = deliver(frame, false);
                out.extend(self.done_with_token(now));
                return out;
            }
            State::PollForMaster if mine && frame.kind == kind::REPLY_TO_POLL_FOR_MASTER => {
                self.next_station = frame.source;
                self.poll_station = me;
                self.token_count = 0;
                return self.pass_token(now);
            }
            _ => {}
        }
        // Anything else heard: someone is speaking, so this master listens.
        self.state = State::Idle;
        self.deadline = now.saturating_add(self.config.no_token);
        if !mine && frame.destination != BROADCAST {
            return Vec::new();
        }
        match frame.kind {
            kind::TOKEN if mine => {
                self.frame_count = 0;
                self.use_token(now)
            }
            kind::POLL_FOR_MASTER if mine => vec![Action::Send(Frame::bare(
                kind::REPLY_TO_POLL_FOR_MASTER,
                frame.source,
                me,
            ))],
            kind::TEST_REQUEST if mine => {
                vec![Action::Send(Frame {
                    kind: kind::TEST_RESPONSE,
                    destination: frame.source,
                    source: me,
                    data: frame.data.clone(),
                })]
            }
            kind::DATA_EXPECTING_REPLY | kind::EXTENDED_EXPECTING_REPLY if mine => {
                self.state = State::AnswerDataRequest;
                self.answering = frame.source;
                self.deadline = now.saturating_add(self.config.reply_delay);
                deliver(frame, false)
            }
            kind::DATA_NOT_EXPECTING_REPLY | kind::EXTENDED_NOT_EXPECTING_REPLY => {
                deliver(frame, !mine)
            }
            _ => Vec::new(),
        }
    }
}
