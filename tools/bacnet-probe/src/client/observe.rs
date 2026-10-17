// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Spans and log lines from the client.
//!
//! The client keeps a stack of open spans. Every request opens its span as a
//! child of whatever is on top — a scan, an API call, a scenario's cycle —
//! so a trace nests the way the work did: the cycle contains the call, the
//! call the reads it made. A caller opens its own span with
//! [`Client::enter`] and closes it with [`Client::exit`].

pub(crate) use super::target::Target;
use super::{Client, ReadError};
use crate::names;
use regalium_bacnet_telemetry::{Attribute, Level, Logger, SpanId, Traced, Tracer, keys};
use std::fmt::Display;
use std::sync::Arc;
use std::time::Instant;

impl Client {
    /// Records spans through `tracer`.
    #[must_use]
    pub fn with_tracer(mut self, tracer: Arc<dyn Tracer>) -> Self {
        self.tracer = tracer;
        self
    }

    /// Writes log lines through `logger`.
    #[must_use]
    pub fn with_logger(mut self, logger: Arc<dyn Logger>) -> Self {
        self.logger = logger;
        self
    }

    /// Remembers what a device said about itself; its requests' spans and
    /// log lines carry it from now on. A device with no name read yet keeps
    /// whatever was learnt before, rather than forgetting it.
    pub fn learn(&mut self, identity: crate::Identity) {
        if identity.name.is_empty() && self.identities.contains_key(&identity.instance) {
            return;
        }
        self.identities.insert(identity.instance, identity);
    }

    /// What `instance` has said about itself, if it has been read.
    #[must_use]
    pub fn identity(&self, instance: u32) -> Option<&crate::Identity> {
        self.identities.get(&instance)
    }

    /// What `instance` has said about itself, or its instance and `name`
    /// alone when it has not been read: the labels for a series about it.
    #[must_use]
    pub fn identity_or(&self, instance: u32, name: &str) -> crate::Identity {
        self.identities
            .get(&instance)
            .cloned()
            .unwrap_or_else(|| crate::Identity {
                name: name.to_owned(),
                ..crate::Identity::unknown(instance)
            })
    }

    /// The span requests are made inside now.
    #[must_use]
    pub fn current(&self) -> SpanId {
        self.stack.last().copied().unwrap_or(SpanId::NONE)
    }

    /// Opens `name` inside the current span; requests made until the
    /// matching [`Client::exit`] are its children.
    pub fn enter(&mut self, name: &'static str) -> SpanId {
        let id = self.tracer.start(name, self.current());
        self.stack.push(id);
        id
    }

    /// Closes `span`, as failed with `error` if given, and any span opened
    /// inside it and left open.
    pub fn exit(&mut self, span: SpanId, error: Option<&dyn Display>) {
        while let Some(top) = self.stack.pop() {
            if top == span {
                break;
            }
            self.tracer.end(top, None);
        }
        self.tracer.end(span, error);
    }

    /// Records `item`'s declared attributes on the current span.
    pub fn record(&self, item: &dyn Traced) {
        let span = self.current();
        item.attributes(&mut |a| self.tracer.attribute(span, a));
    }

    /// Records one attribute on the current span.
    pub fn set(&self, attribute: Attribute<'_>) {
        self.tracer.attribute(self.current(), attribute);
    }

    /// Writes a log line inside the current span. Unless the caller labels
    /// it, it carries the identity of the device the latest request went
    /// to, so "every warning from firmware 11" is a filter on any line.
    pub fn log(&self, level: Level, message: &dyn Display, attributes: &[Attribute<'_>]) {
        if !self.logger.enabled(level) {
            return;
        }
        let identity = self.last_device.and_then(|d| self.identities.get(&d));
        match identity.filter(|_| attributes.is_empty()) {
            Some(identity) => {
                self.logger
                    .log(level, message, self.current(), &identity.attribute_list());
            }
            None => self.logger.log(level, message, self.current(), attributes),
        }
    }

    /// Runs one request inside its own span: what it was about, how it
    /// ended, and a debug line saying so; a warning when the device was
    /// silent, which a reader should notice and a refusal need not be.
    pub(crate) fn request<T>(
        &mut self,
        name: &'static str,
        target: &Target,
        f: impl FnOnce(&mut Self) -> Result<T, ReadError>,
    ) -> Result<T, ReadError> {
        let span = self.enter(name);
        self.last_device = Some(target.device);
        self.record(target);
        if let Some(identity) = self.identities.get(&target.device) {
            self.record(identity);
        }
        let started = Instant::now();
        let result = f(self);
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        let outcome = result.as_ref().map_or_else(ReadError::outcome, |_| "ok");
        self.set(Attribute::str(keys::OUTCOME, outcome));
        if let Err(ReadError::Bacnet { class, code }) = &result {
            let (class, code) = (names::error_class(*class), names::error_code(*code));
            self.set(Attribute::str(keys::ERROR_CLASS, &class));
            self.set(Attribute::str(keys::ERROR_CODE, &code));
        }
        let short = name.trim_start_matches("bacnet.client.");
        let mut attrs = target.attribute_list();
        if let Some(identity) = self.identities.get(&target.device) {
            attrs.extend(identity.attribute_list());
        }
        match &result {
            Err(e @ (ReadError::Timeout | ReadError::Io(_))) => {
                self.log(
                    Level::Warn,
                    &format_args!("{short} {target}: {e} after {elapsed:.1} ms"),
                    &attrs,
                );
            }
            Err(e) => self.log(
                Level::Debug,
                &format_args!("{short} {target}: refused, {e} ({elapsed:.1} ms)"),
                &attrs,
            ),
            Ok(_) => self.log(
                Level::Debug,
                &format_args!("{short} {target}: ok in {elapsed:.1} ms"),
                &attrs,
            ),
        }
        self.exit(span, result.as_ref().err().map(|e| e as &dyn Display));
        result
    }
}
