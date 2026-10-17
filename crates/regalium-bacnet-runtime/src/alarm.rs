// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Event notifications (§13.8, §13.9): alarms a device sends to the client
//! as a recipient of a notification class, acknowledged when confirmed and
//! kept until the caller takes them.

use crate::client::Client;
use crate::device::Route;
use regalium_bacnet_core::apdu::{Apdu, Pdu};
use regalium_bacnet_core::datalink::Datalink;
use regalium_bacnet_core::primitive::ObjectIdentifier;
use regalium_bacnet_core::service::{EventNotification, TimeStamp, confirmed, unconfirmed};

/// An event a device reported, owned.
///
/// Plain data: `Send` and `Sync`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Alarm {
    /// The device reporting.
    pub device: u32,
    /// The object the event is about.
    pub object: ObjectIdentifier,
    /// When it happened, by the device's clock or count.
    pub time: TimeStamp,
    /// The notification class.
    pub class: u32,
    /// 0 most urgent to 255.
    pub priority: u8,
    /// `BACnetEventType`: the algorithm that raised it.
    pub event_type: u32,
    /// 0 alarm, 1 event, 2 acknowledgement notification.
    pub notify_type: u32,
    /// The device's text for it, when it sent one in UTF-8 or a set this
    /// build reads.
    pub message: Option<String>,
    /// Whether an operator must acknowledge it.
    pub ack_required: bool,
    /// The event state it left, when said.
    pub from_state: Option<u32>,
    /// The event state it entered.
    pub to_state: u32,
    /// The event algorithm's values, as sent.
    pub values: Option<Vec<u8>>,
}

impl<D: Datalink> Client<D> {
    /// The alarms since the last call, oldest first. Past
    /// [`Config::events`](crate::Config::events) the oldest were dropped.
    pub fn alarms(&mut self) -> impl Iterator<Item = Alarm> + '_ {
        self.alarms.drain(..)
    }

    /// Handles `apdu` when it is an event notification, returning whether
    /// it was. A confirmed one is acknowledged (§13.8) before it is kept.
    pub(crate) fn alarmed(&mut self, from: D::Mac, route: Option<&Route>, apdu: &Apdu<'_>) -> bool {
        let (body, ack) = match apdu.pdu {
            Pdu::UnconfirmedRequest {
                service: unconfirmed::EVENT_NOTIFICATION,
                body,
            } => (body, None),
            Pdu::ConfirmedRequest {
                service: confirmed::EVENT_NOTIFICATION,
                invoke_id,
                segment: None,
                body,
                ..
            } => (body, Some(invoke_id)),
            _ => return false,
        };
        if let Some(invoke) = ack {
            self.acknowledge(from, route, invoke, confirmed::EVENT_NOTIFICATION);
        }
        let Ok(n) = EventNotification::decode(body) else {
            return true;
        };
        let alarm = Alarm {
            device: n.device.instance,
            object: n.object,
            time: n.time,
            class: n.class,
            priority: n.priority,
            event_type: n.event_type,
            notify_type: n.notify_type,
            message: n
                .message
                .and_then(|m| m.text().map(std::borrow::Cow::into_owned)),
            ack_required: n.ack_required == Some(true),
            from_state: n.from_state,
            to_state: n.to_state,
            values: n.values.map(<[u8]>::to_vec),
        };
        while self.config.events > 0 && self.alarms.len() >= self.config.events {
            self.alarms.pop_front();
        }
        if self.config.events > 0 {
            self.alarms.push_back(alarm);
        }
        true
    }
}
