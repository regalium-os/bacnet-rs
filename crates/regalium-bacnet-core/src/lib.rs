// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The pure `BACnet` protocol core: bytes in, decisions and deadlines out.
//!
//! Implements ASHRAE 135 / ISO 16484-5 from the tag codec up to the
//! transaction state machine. It performs no I/O, opens no socket and reads no
//! clock — it is handed octets and a timestamp and returns what should happen
//! next. That is what lets a full request, retry, segmentation and timeout
//! scenario run as a table test on a fake clock, with no hardware and no
//! network.
//!
//! # Layer contract
//!
//! Owns, layering inward:
//!
//! | Module | Owns |
//! | --- | --- |
//! | `tag` | ASHRAE 135 §20.2 tag encoding: application and context tags, extended tag numbers, extended lengths, opening and closing tags |
//! | `primitive` | the 13 primitive application types (§20.2.2–§20.2.14) |
//! | `constructed` | sequences, choices, arrays and lists built from them |
//! | `enumerated` | object types, property identifiers, error classes, engineering units |
//! | `apdu` | the eight PDU types (§20.1.2–§20.1.9) |
//! | `npdu` | network layer: NPCI, DNET/DADR/SNET/SADR, hop count, network messages (Clause 6) |
//! | `service` | confirmed, unconfirmed and acknowledgement service codecs (Clauses 13–17) |
//! | `obj` | the object model: identifiers, property references, the database trait |
//! | `tsm` | the transaction state machine: invoke ids, retries, segmentation |
//!
//! Must not: perform I/O, hold an operating-system handle, consult a real
//! clock, log, or panic. Malformed input is the normal case on a shared wire;
//! it is an error return, never an abort.
//!
//! Allowed imports: `regalium-bacnet-telemetry`.
//!
//! # Decoding is lossless
//!
//! Decoding then re-encoding reproduces the input byte for byte, **including a
//! malformed length or a reserved bit**, so traffic from equipment that does
//! not follow the standard can be inspected rather than silently normalised.
//! A driver that quietly corrects what a lift controller sent is a driver that
//! hides the reason the site does not work.
//!
//! # Spans
//!
//! `bacnet.tag.{decode,encode}`, `bacnet.apdu.{decode,encode}`,
//! `bacnet.npdu.{decode,route}`, `bacnet.service.<service>`,
//! `bacnet.tsm.transaction`.
