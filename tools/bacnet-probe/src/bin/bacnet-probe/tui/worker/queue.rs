// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The command queue: what arrived while a job ran, and whether it should
//! cut the job short.

use super::{Command, Job, Worker};
use std::sync::mpsc::TryRecvError;

impl Worker {
    /// Drains new commands, then decides whether `job` should stop. While
    /// loading a device, the newest detail request is served right here, so
    /// browsing objects never waits for the whole list.
    pub(super) fn superseded(&mut self, job: Job) -> bool {
        self.drain();
        if let Job::Device(instance) = job {
            self.pending
                .retain(|c| !matches!(c, Command::LoadDevice(d) if d.instance == instance));
            if let Some(i) = self
                .pending
                .iter()
                .rposition(|c| matches!(c, Command::LoadDetail(..)))
            {
                let newest = self.pending.remove(i);
                self.pending
                    .retain(|c| !matches!(c, Command::LoadDetail(..)));
                if let Some(Command::LoadDetail(device, id)) = newest {
                    self.load_detail(&device, id);
                }
            }
        }
        self.closed
            || self.pending.iter().any(|c| match (job, c) {
                (Job::Device(_), Command::LoadDetail(..)) => false,
                (Job::Device(_) | Job::Detail | Job::Live, _) => true,
            })
    }

    /// Moves new commands into `pending`; watch and interval changes apply
    /// immediately because they never need to wait their turn.
    pub(super) fn drain(&mut self) {
        loop {
            match self.commands.try_recv() {
                Ok(Command::Watch(w)) => self.live.want(w),
                Ok(Command::SetInterval(d)) => self.live.interval = d,
                Ok(c) => self.pending.push_back(c),
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    self.closed = true;
                    return;
                }
            }
        }
    }
}
