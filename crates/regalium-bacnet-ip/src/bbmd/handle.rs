// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! What a BBMD does with each message it receives (Annex J.4.5, J.5.2).

use super::{Bbmd, Foreign, GRACE, Out, Target};
use crate::bvll::{Bdt, Bvll, Fdt, FdtEntry, Function, result};
use core::time::Duration;
use std::net::SocketAddrV4;

/// What handling one message produced.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Handled {
    /// Messages to send.
    pub out: Vec<Out>,
    /// Whether its NPDU is for this node too, from whom, and whether it was
    /// a broadcast. The NPDU is [`Bvll::npdu`] of the message handled.
    pub deliver: Option<(SocketAddrV4, bool)>,
}

impl Handled {
    fn reply(to: SocketAddrV4, function: Function<'_>) -> Self {
        Self {
            out: vec![Out {
                to: Target::Unicast(to),
                bvll: Bvll::to_vec(function),
            }],
            deliver: None,
        }
    }

    fn result(to: SocketAddrV4, code: u16) -> Self {
        Self::reply(to, Function::Result(code))
    }
}

impl Bbmd {
    /// Handles `bvll` from `from` at `now`. A message from this node itself
    /// — its own broadcast, heard back — does nothing.
    pub fn handle(&mut self, from: SocketAddrV4, bvll: &Bvll<'_>, now: Duration) -> Handled {
        if from == self.me {
            return Handled::default();
        }
        match bvll.function {
            Function::OriginalUnicast(_) => Handled {
                out: Vec::new(),
                deliver: Some((from, false)),
            },
            Function::OriginalBroadcast(npdu) => Handled {
                out: self.relay(from, npdu, true, true, None),
                deliver: Some((from, true)),
            },
            Function::Forwarded { origin, npdu } => {
                let mut out = self.relay(origin, npdu, false, true, None);
                if self.two_hop() {
                    out.insert(0, local(origin, npdu));
                }
                Handled {
                    out,
                    deliver: Some((origin, true)),
                }
            }
            Function::DistributeBroadcast(npdu) => {
                if !self.fdt.iter().any(|f| f.address == from) {
                    return Handled::result(from, result::DISTRIBUTE_BROADCAST_NAK);
                }
                let mut out = vec![local(from, npdu)];
                out.extend(self.relay(from, npdu, true, true, Some(from)));
                Handled {
                    out,
                    deliver: Some((from, true)),
                }
            }
            Function::RegisterForeignDevice { ttl } => self.register(from, ttl, now),
            Function::ReadBdt => {
                let mut table = Vec::new();
                self.bdt.iter().for_each(|e| e.encode(&mut table));
                Handled::reply(from, Function::ReadBdtAck(Bdt(&table)))
            }
            Function::WriteBdt(table) if self.writable_bdt => {
                self.bdt = table.iter().collect();
                Handled::result(from, result::SUCCESS)
            }
            Function::WriteBdt(_) => Handled::result(from, result::WRITE_BDT_NAK),
            Function::ReadFdt => {
                let mut table = Vec::new();
                for f in &self.fdt {
                    let left = f.expires.saturating_sub(now).as_secs();
                    FdtEntry {
                        address: f.address,
                        ttl: f.ttl,
                        remaining: u16::try_from(left).unwrap_or(u16::MAX),
                    }
                    .encode(&mut table);
                }
                Handled::reply(from, Function::ReadFdtAck(Fdt(&table)))
            }
            Function::DeleteFdtEntry(address) => {
                let before = self.fdt.len();
                self.fdt.retain(|f| f.address != address);
                let code = if self.fdt.len() < before {
                    result::SUCCESS
                } else {
                    result::DELETE_FDT_ENTRY_NAK
                };
                Handled::result(from, code)
            }
            Function::Result(_)
            | Function::ReadBdtAck(_)
            | Function::ReadFdtAck(_)
            | Function::SecureBvll(_)
            | Function::Unknown { .. } => Handled::default(),
        }
    }

    fn register(&mut self, from: SocketAddrV4, ttl: u16, now: Duration) -> Handled {
        if !self.accept_foreign {
            return Handled::result(from, result::REGISTER_FOREIGN_DEVICE_NAK);
        }
        let expires = now
            .saturating_add(Duration::from_secs(ttl.into()))
            .saturating_add(GRACE);
        self.fdt.retain(|f| f.address != from);
        self.fdt.push(Foreign {
            address: from,
            ttl,
            expires,
        });
        Handled::result(from, result::SUCCESS)
    }
}

/// A Forwarded-NPDU on the local subnet, so every station hears `origin`'s
/// broadcast with `origin` as its sender.
fn local(origin: SocketAddrV4, npdu: &[u8]) -> Out {
    Out {
        to: Target::LocalBroadcast,
        bvll: Bvll::to_vec(Function::Forwarded { origin, npdu }),
    }
}
