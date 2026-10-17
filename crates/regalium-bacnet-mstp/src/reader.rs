// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Frames out of a byte stream (§9.5.4): an RS-485 line delivers octets as
//! they come, noise included, and a frame is whatever starts with the
//! preamble and checks.

use crate::frame::{Frame, FrameError};

/// The longest frame worth waiting for: an extended frame of the largest
/// NPDU, encoded. A header claiming more is noise that happened to check.
const LONGEST: usize = 8 + 2048;

/// Accumulates octets and yields frames.
///
/// Plain data: `Send` and `Sync`. One reader per line.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reader {
    buf: Vec<u8>,
    discarded: usize,
}

impl Reader {
    /// A reader with nothing in it.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            buf: Vec::new(),
            discarded: 0,
        }
    }

    /// How many frames were thrown away for a bad check or encoding.
    #[must_use]
    pub const fn discarded(&self) -> usize {
        self.discarded
    }

    /// Adds `octets`, returning every frame now complete.
    pub fn push(&mut self, octets: &[u8]) -> Vec<Frame> {
        self.buf.extend_from_slice(octets);
        let mut frames = Vec::new();
        loop {
            // Skip to the next preamble; a lone X'55' at the end may be one.
            let Some(start) = self.buf.windows(2).position(|w| w == [0x55, 0xFF]) else {
                let keep = usize::from(self.buf.last() == Some(&0x55));
                let len = self.buf.len();
                self.buf.drain(..len - keep);
                return frames;
            };
            self.buf.drain(..start);
            match Frame::decode(&self.buf) {
                Ok((frame, used)) => {
                    self.buf.drain(..used);
                    frames.push(frame);
                }
                Err(FrameError::Truncated { needed }) if needed <= LONGEST => return frames,
                Err(_) => {
                    self.discarded += 1;
                    self.buf.drain(..1);
                }
            }
        }
    }
}
