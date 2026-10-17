// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Logs: what an application layer tells a person, at a level, attached to
//! the span it happened in.
//!
//! The pure core never logs — it returns errors and spans, and a library
//! that narrates into someone else's output is one they cannot embed. The
//! runtime and the applications built on the driver do log, and they do it
//! here, so a log line and the trace it belongs to arrive together and a
//! reader can go from one to the other.

use crate::attribute::Attribute;
use crate::trace::SpanId;
use core::fmt::Display;

/// How much a line matters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Every request and reply: what an engineer turns on to see why.
    Debug,
    /// What happened, at the pace a person reads: a device found, a cycle done.
    Info,
    /// Something went wrong that the program worked around: a device that
    /// refused, a timeout before a retry succeeded.
    Warn,
    /// Something went wrong that it could not: a value that did not read
    /// back as written.
    Error,
}

impl Level {
    /// The level's name, as a log pipeline expects it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

/// Where log lines go.
///
/// # Concurrency
///
/// `Send + Sync`; lines may arrive from any thread, and are kept in the
/// order each thread sends them.
pub trait Logger: Send + Sync {
    /// Whether a line at `level` would be kept, so a caller can skip
    /// building one nobody reads.
    fn enabled(&self, level: Level) -> bool;

    /// Records `message` at `level`, inside `span` (or none), with
    /// attributes under declared keys only. The message is formatted only
    /// if the line is kept.
    fn log(&self, level: Level, message: &dyn Display, span: SpanId, attributes: &[Attribute<'_>]);
}

/// Keeps nothing.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopLogger;

impl Logger for NoopLogger {
    fn enabled(&self, _: Level) -> bool {
        false
    }

    fn log(&self, _: Level, _: &dyn Display, _: SpanId, _: &[Attribute<'_>]) {}
}
