# Roadmap

Everything bacnet-rs is to be, crate by crate: what is built, what is next,
and the order it comes in. The [README](../README.md) is the summary; each
crate's `lib.rs` layer contract is the authority on what it owns.

**Built** means merged, tested, and run in CI. Everything else is designed,
not written.

## How a phase ends

Each phase ends with a committed corpus of hex-dumped BACnet traffic that
decodes and re-encodes byte for byte, malformed records included. **No phase
starts before the previous corpus passes.** `just fixtures` is the gate; it
says so rather than passing while no corpus exists.

## Phases

The numbers are the ones the repository already uses. Work after Phase 9 is
listed in order without inventing numbers for it.

| Phase | Delivers | State |
| --- | --- | --- |
| 0 | the workspace, layer contracts, conformance gates with negative tests, CI | **built** |
| — | the telemetry seam's `Meter` and its SDK binding, ahead of the spans | **built** |
| — | the field probe, on a codec of its own | **built** |
| 3 | the tag codec and the primitive types; the first spans and fixture corpus | next |
| 4 | APDU and NPDU | |
| 5 | the transaction state machine: invoke ids, retries, segmentation | |
| 6 | `ReadProperty` and `Who-Is` / `I-Am` | |
| 7 | BACnet/IP, Annex J | |
| 8 | the first live read, end to end; the probe moves onto the driver | |
| 9 | the schema tree, `protobuf/`, under the AIP linter | **built** early |
| — | the breadth of types, services and the object model | |
| — | MS/TP, BACnet/SC and BACnet/IPv6 | |
| 17 | fuzz targets for every decoder | |

## The pure core

`regalium-bacnet-core` — bytes and a timestamp in, decisions and deadlines
out. No I/O, no clock, no panic.

- **Tags** (§20.2): application and context tags, extended tag numbers and
  lengths, opening and closing tags.
- **Primitive types** (§20.2.2–§20.2.14), all thirteen, and the
  **constructed** sequences, choices, arrays and lists built from them.
- **Enumerations**: object types, property identifiers, error classes,
  engineering units.
- **APDU**: the eight PDU types (§20.1.2–§20.1.9).
- **NPDU** (Clause 6): NPCI, DNET/DADR/SNET/SADR, hop count, network layer
  messages.
- **Services** (Clauses 13–17): confirmed, unconfirmed and acknowledgement
  codecs — first those [usage.md](usage.md) is built to: `Who-Is`/`I-Am`,
  `Who-Has`/`I-Have`, `ReadProperty`, `ReadPropertyMultiple`, `SubscribeCOV`,
  `SubscribeCOVProperty` and COV and event notifications.
- **Object model**: identifiers, property references, the database trait a
  local device serves from.
- **Transaction state machine**: invoke ids, retry, segmentation, abandon —
  as a table test on a fake clock.
- **Lossless decoding**: re-encoding reproduces the input, malformed length
  and reserved bits included.

## Datalinks

Each satisfies the datalink trait the core declares, so a proprietary bridge
is implemented outside this repository. None interprets an APDU.

- **BACnet/IP** (`regalium-bacnet-ip`, Annex J): BVLL, directed and broadcast
  distribution, a BBMD's broadcast distribution table, Forwarded-NPDU, the
  foreign device table, and foreign device registration with re-registration
  before it lapses. This is what the Schindler Cube exposes, and through its
  routing, what reaches every lift controller behind it.
- **MS/TP** (`regalium-bacnet-mstp`, Clause 9): the master node state
  machine over RS-485 — token passing, poll-for-master, sole master — and
  the frame codec with its 8-bit header and 16-bit data CRCs. Every timer is
  a returned deadline, so token loss and a duplicate master test without a
  bus.
- **BACnet/SC** (`regalium-bacnet-sc`, Annex AB): BVLC-SC over WebSocket and
  TLS, hub-and-spoke and direct connect, certificate identity, and the
  heartbeat that tells a quiet peer from a gone one. Never records key
  material.
- **BACnet/IPv6** (`regalium-bacnet-ipv6`, Annex U): multicast instead of
  broadcast, virtual MAC addresses and their resolution, BBMD and foreign
  devices. Its full feature list is **[ipv6.md](ipv6.md)**.

## Runtime and surface

- **Runtime** (`regalium-bacnet-runtime`): the only thing that waits. Device
  discovery and binding, the poll scheduler, the COV subscription lifecycle
  with renewal before lapse, reconnection with backoff, `ReadPropertyMultiple`
  fallback remembered per device, and the event stream through which a
  device going offline is reported — an event, not an error. `new` spawns
  nothing; `run` returns only when its tasks have stopped; `close` is
  idempotent.
- **Facade** (`regalium-bacnet`): the public surface, all `pub use`. The API
  it is built to is in **[usage.md](usage.md)**: `Client`, discovery,
  reading, subscribing, serving a `LocalDevice`, and driving the core
  directly.
- **Schema** (`regalium-bacnet-schema`): devices, objects, points and events
  in `protobuf/`, resource-oriented per Google AIP, with prost types and
  generated FlatBuffers and Cap'n Proto mirrors of the hot-path payloads.
  `buffers.lock` pins every field's ordinal, because a moved slot is a wire
  break no compiler reports.
- **Vendor profiles**: the revision-12 and revision-19 Schindler object
  models as two profiles over one codec. A vendor never gets its own framing;
  a quirk is a flag in the codec with a fixture proving it.

## Telemetry

- **Built**: the seam's `Meter`, instruments, attributes and declared keys
  (`regalium-bacnet-telemetry`, no dependencies), and its binding to the
  the-protobuf-project SDK with OTLP export (`regalium-bacnet-telemetry-otel`).
- **Next**, with the first spans in Phase 3: the tracer and logger traits,
  and `trace_attrs!`, through which an undeclared field cannot be recorded.
  Spans are `bacnet.<layer>.<operation>`; each crate's contract lists its own.

## The field probe

**Built**, in `tools/bacnet-probe`, and shipped by Homebrew and release
archives. Over BACnet/IP: Who-Is discovery by broadcast, swept ranges or
unicast, through a BBMD as a foreign device, and through routers to MS/TP;
Who-Has; `ReadProperty` and `ReadPropertyMultiple` with fallback and request
halving; `WriteProperty` at a priority, `WritePropertyMultiple`,
`CreateObject` and `DeleteObject`; the schema's device, object and property
services, served over its client and reachable as `get`, `list`, `write`,
`create` and `delete` by resource name; COV with renewal and polling for what
refuses; JSON and table output;
OTLP metrics; an offline dissector; a simulated gateway; and a TUI with
Browse, Packets, Log and Settings tabs, where the network options are edited
and applied live. See [its README](../tools/bacnet-probe/README.md).

Its codec is deleted in Phase 8, when it moves onto the driver; IPv6 and the
other datalinks then reach it without probe-specific work.

## The gates

**Built** in `xtask`, each with a negative test that plants a violation:
file size, licence notices, purity of the core, layering, and the dependency
graph — no OpenTelemetry outside the adapter, no C beneath the driver beyond
the one audited exception. Every gate fails when it examined nothing.
