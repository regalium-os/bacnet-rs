// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! COV subscriptions (§13.14) and the notifications they bring (§13.6,
//! §13.7): subscribe, renew before the lifetime lapses, cancel, and turn
//! each notification into an update — acknowledging a confirmed one.

use crate::batch::{Answer, Call};
use crate::client::Client;
use crate::device::Route;
use crate::error::RequestError;
use crate::reading::Reading;
use crate::watch::{Source, Update, Watch};
use regalium_bacnet_core::apdu::{Apdu, Pdu};
use regalium_bacnet_core::datalink::{Datalink, Recipient};
use regalium_bacnet_core::npdu::{Address, Body, Destination, Npdu};
use regalium_bacnet_core::service::{
    CovNotification, ReadProperty, SubscribeCov, confirmed, unconfirmed,
};
use std::time::Duration;

/// When to renew a subscription of `lifetime`: 30 s early when it allows,
/// half way when it is short, so a renewal that is itself lost has time to
/// be retried before the device forgets.
fn renew_after(lifetime: Duration) -> Duration {
    let early = if lifetime > Duration::from_secs(60) {
        lifetime.saturating_sub(Duration::from_secs(30))
    } else {
        lifetime / 2
    };
    early.max(Duration::from_secs(1))
}

impl<D: Datalink> Client<D> {
    /// Subscribes watch `id`'s object for `lifetime`, unconfirmed, with
    /// `id` as the subscriber process identifier. On success, how long until
    /// the renewal is due.
    pub(crate) fn subscribe(
        &mut self,
        id: u32,
        watch: &Watch,
        lifetime: Duration,
    ) -> Result<Result<Duration, RequestError>, D::Error> {
        let request = SubscribeCov {
            process: id,
            object: watch.object,
            confirmed: Some(false),
            lifetime: Some(u32::try_from(lifetime.as_secs()).unwrap_or(u32::MAX).max(1)),
        };
        let answer = self.cov_call(watch.device, &request, "bacnet.runtime.subscribe")?;
        Ok(answer.map(|_| renew_after(lifetime)))
    }

    /// Cancels watch `id`'s subscription: `SubscribeCOV` without a lifetime.
    pub(crate) fn cancel(&mut self, id: u32, watch: &Watch) -> Result<Answer, D::Error> {
        let request = SubscribeCov {
            process: id,
            object: watch.object,
            confirmed: None,
            lifetime: None,
        };
        self.cov_call(watch.device, &request, "bacnet.runtime.unsubscribe")
    }

    fn cov_call(
        &mut self,
        device: u32,
        request: &SubscribeCov,
        span: &'static str,
    ) -> Result<Answer, D::Error> {
        let mut body = Vec::new();
        request.encode(&mut body);
        let call = Call {
            device,
            service: confirmed::SUBSCRIBE_COV,
            body,
        };
        Ok(self
            .transact(&[call], span)?
            .pop()
            .unwrap_or(Err(RequestError::Unanswered)))
    }

    /// Handles `apdu` when it is a COV notification, returning whether it
    /// was: a confirmed one is acknowledged (§13.7) whoever it is for, and
    /// one for a watch becomes an update.
    pub(crate) fn notified(
        &mut self,
        from: D::Mac,
        route: Option<&Route>,
        apdu: &Apdu<'_>,
    ) -> bool {
        let (body, ack) = match apdu.pdu {
            Pdu::UnconfirmedRequest {
                service: unconfirmed::COV_NOTIFICATION,
                body,
            } => (body, None),
            Pdu::ConfirmedRequest {
                service: confirmed::COV_NOTIFICATION,
                invoke_id,
                segment: None,
                body,
                ..
            } => (body, Some(invoke_id)),
            _ => return false,
        };
        if let Some(invoke) = ack {
            self.acknowledge(from, route, invoke, confirmed::COV_NOTIFICATION);
        }
        let Ok(n) = CovNotification::decode(body) else {
            return true;
        };
        let device = n.device.instance;
        let matches = self
            .watches
            .get(&n.process)
            .is_some_and(|w| w.watch.device == device && w.watch.object == n.object);
        if matches {
            let values = n
                .values
                .iter()
                .map(|v| {
                    let read = ReadProperty {
                        object: n.object,
                        property: v.property.property,
                        index: v.property.index,
                    };
                    Ok(Reading {
                        device,
                        read,
                        value: v.value.to_vec(),
                    })
                })
                .collect();
            self.reached(device);
            self.update(Update {
                watch: n.process,
                device,
                object: n.object,
                source: Source::Cov,
                values,
            });
        }
        true
    }

    /// Sends a Simple-ACK for confirmed request `invoke` of `service` to
    /// `from`, routed back the way it came. A lost acknowledgement is
    /// resent by the device, so a send that fails is not reported.
    pub(crate) fn acknowledge(&self, from: D::Mac, route: Option<&Route>, invoke: u8, service: u8) {
        let simple_ack = [0x20, invoke, service];
        let destination = route.map(|r| Destination {
            address: Address {
                network: r.network,
                mac: &r.mac,
            },
            hop_count: 255,
        });
        let npdu = Npdu {
            control: 0,
            destination,
            source: None,
            body: Body::Apdu(&simple_ack),
        };
        let mut out = Vec::new();
        npdu.encode(&mut out);
        let _ = self.link.send(Recipient::Unicast(from), &out);
    }
}
