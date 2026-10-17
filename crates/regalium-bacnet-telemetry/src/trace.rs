// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Spans: the tracer every layer records through, and the guard that ends
//! a span however its scope is left.
//!
//! A span is a number. The tracer hands one out on start, and attributes and
//! the end refer to it, so the seam carries no span object and the no-op
//! tracer allocates nothing, holds nothing, and costs three calls.

use crate::attribute::Attribute;
use crate::traced::Traced;
use core::fmt::Display;

/// A span, as the tracer that started it knows it. [`SpanId::NONE`] is what
/// a tracer that records nothing returns, and is ignored on every call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpanId(pub u64);

impl SpanId {
    /// Not recording.
    pub const NONE: Self = Self(0);
}

/// Where spans go. Implemented by the adapter that binds the telemetry SDK,
/// and by [`NoopTracer`].
///
/// # Concurrency
///
/// `Send + Sync`: a layer holds `&dyn Tracer` and may record from any
/// thread. An implementation must accept `attribute` and `end` for a span
/// from a thread other than the one that started it.
pub trait Tracer: Send + Sync {
    /// Starts a span named `bacnet.<layer>.<operation>`, as a child of
    /// `parent`, or as the root of a new trace when `parent` is
    /// [`SpanId::NONE`]. Nesting is what turns a trace into a story: a scan
    /// that contains the reads it made, each containing its retries.
    fn start(&self, name: &'static str, parent: SpanId) -> SpanId;

    /// Adds one attribute. Built only from a declared key, as every
    /// [`Attribute`] is.
    fn attribute(&self, span: SpanId, attribute: Attribute<'_>);

    /// Ends the span, as failed when `error` is given.
    fn end(&self, span: SpanId, error: Option<&dyn Display>);
}

/// Records nothing, at the cost of three empty calls.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopTracer;

impl Tracer for NoopTracer {
    fn start(&self, _: &'static str, _: SpanId) -> SpanId {
        SpanId::NONE
    }

    fn attribute(&self, _: SpanId, _: Attribute<'_>) {}

    fn end(&self, _: SpanId, _: Option<&dyn Display>) {}
}

/// A started span, ended when dropped unless [`Span::fail`] or
/// [`Span::end`] ended it first, so an early return cannot leak one.
pub struct Span<'t> {
    tracer: &'t dyn Tracer,
    id: SpanId,
    open: bool,
}

impl<'t> Span<'t> {
    /// Starts `name` on `tracer`, as the root of a new trace.
    #[must_use]
    pub fn start(tracer: &'t dyn Tracer, name: &'static str) -> Self {
        Self::within(tracer, name, SpanId::NONE)
    }

    /// Starts `name` as a child of `parent`.
    #[must_use]
    pub fn within(tracer: &'t dyn Tracer, name: &'static str, parent: SpanId) -> Self {
        Self {
            tracer,
            id: tracer.start(name, parent),
            open: true,
        }
    }

    /// Starts `name` as a child of this span.
    #[must_use]
    pub fn child(&self, name: &'static str) -> Self {
        Self::within(self.tracer, name, self.id)
    }

    /// This span's id, to parent a span started elsewhere or to correlate a
    /// log line with it.
    #[must_use]
    pub fn id(&self) -> SpanId {
        self.id
    }

    /// Records one attribute, under a declared key.
    pub fn set(&self, attribute: Attribute<'_>) {
        if self.id != SpanId::NONE {
            self.tracer.attribute(self.id, attribute);
        }
    }

    /// Records every attribute `item` declares with `trace_attrs!`, and
    /// nothing it does not.
    pub fn record(&self, item: &dyn Traced) {
        if self.id != SpanId::NONE {
            item.attributes(&mut |a| self.tracer.attribute(self.id, a));
        }
    }

    /// Ends the span as failed with `error`.
    pub fn fail(mut self, error: &dyn Display) {
        self.open = false;
        self.tracer.end(self.id, Some(error));
    }

    /// Ends the span as succeeded.
    pub fn end(mut self) {
        self.open = false;
        self.tracer.end(self.id, None);
    }
}

impl Drop for Span<'_> {
    fn drop(&mut self) {
        if self.open {
            self.tracer.end(self.id, None);
        }
    }
}
