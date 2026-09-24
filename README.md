# bacnet-rs

A pure-Rust implementation of **BACnet** — ASHRAE 135 / ISO 16484-5 — the
building automation protocol that HVAC plant, lighting, metering, access
control and lift equipment speak to each other.

It is a library and a runtime, not an application: no global state, no hidden
threads, and no clock the core reads behind your back.

> **Status: Phase 0.** The workspace, the layer contracts and the conformance
> gates exist and are enforced. **No frame decodes yet.** The code below is the
> target API, written down so its shape can be argued with before it is built.
> The tag codec and the first fixture corpus land in Phase 3; the first live
> read in Phase 8. See [State](#state).

## Why another BACnet stack

BACnet stacks tend to fuse the codec to a socket and a clock. That makes the
interesting failures — a retry storm, a segmented response that never
completes, a device that goes quiet mid-transaction — reproducible only against
hardware, which means they are not reproduced at all.

**The core is sans-io.** It is handed octets and a timestamp and returns what
should happen next:

```rust
match tsm.poll(clock.now()) {
    Step::Transmit { bytes, deadline } => port.send(bytes, deadline)?,
    Step::Retry { attempt, bytes, .. } => port.send(bytes, /* ... */)?,
    Step::Abandon { invoke, reason } => report_timeout(invoke, reason),
    Step::Idle { wake_at } => clock.park_until(wake_at),
}
```

Nothing there opens a socket or reads a clock, so a full timeout, retry,
segmentation and abandon sequence is a table test on a fake clock — no
hardware, no network, no sleeping test suite. The runtime is the only thing
that waits.

**Decoding is lossless.** Decode then re-encode reproduces the input byte for
byte, *including* a malformed length or a reserved bit:

```rust
let (npdu, apdu_bytes) = Npdu::decode(&bvlc.payload)?;
let apdu = Apdu::decode(apdu_bytes)?;

assert_eq!(apdu.encode_to_vec(), apdu_bytes);
```

Equipment that does not follow the standard is the normal case. A stack that
quietly normalises what a controller actually sent hides the reason a site does
not work.

## A first look

```rust
use regalium_bacnet::{Client, ObjectType, PropertyId};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), regalium_bacnet::Error> {
    let client = Client::bind("0.0.0.0:47808").await?;

    // Who-Is / I-Am: the exchange every BACnet network runs on.
    for device in client.who_is().within(Duration::from_secs(3)).collect().await? {
        println!("{} — {} at {}", device.instance(), device.name(), device.address());
    }

    let device = client.bind_device(599).await?;
    let temperature: f32 = client
        .read_property(device, (ObjectType::AnalogInput, 1), PropertyId::PresentValue)
        .await?
        .try_into()?;

    // Push rather than poll; the subscription's lifetime is renewed for you.
    let mut changes = client
        .subscribe_cov(device, (ObjectType::AnalogInput, 1))
        .lifetime(Duration::from_secs(300))
        .await?;
    while let Some(change) = changes.next().await {
        println!("{} = {}", change.property(), change.value());
    }
    Ok(())
}
```

`ReadPropertyMultiple`, serving as a local device, driving the core without the
runtime, and the other datalinks are in **[docs/usage.md](docs/usage.md)**.

## Architecture

The workspace is the layering. Each crate may depend only on the crates above
it, and `cargo xtask arch` reads the resolved dependency graph to prove it — a
module boundary is a convention, a crate boundary is a fact the compiler
already knows.

| Crate | Owns | Pure |
| --- | --- | --- |
| `regalium-bacnet-telemetry` | tracer, meter and logger traits; no dependencies at all | ✅ |
| `regalium-bacnet-core` | tags, primitive and constructed types, APDU, NPDU, services, object model, transaction state machine | ✅ |
| `regalium-bacnet-ip` | BACnet/IP — Annex J: BVLL, BBMD, foreign device registration | |
| `regalium-bacnet-mstp` | MS/TP master — Clause 9, over RS-485 | |
| `regalium-bacnet-sc` | BACnet Secure Connect — Annex AB, over WebSocket and TLS | |
| `regalium-bacnet-ipv6` | BACnet/IPv6 — Annex U | |
| `regalium-bacnet-schema` | generated domain models and their mirrors | |
| `regalium-bacnet-runtime` | the clock, sockets, discovery, polling, subscription lifecycle | |
| `regalium-bacnet` | the public surface — `pub use`, never a newtype | |

**Pure** means no I/O and no clock. A re-export is the same type, so
implementing a datalink for a proprietary bridge, or adding an object type,
needs no change here; a wrapper type would compile and then silently close
every extension point.

## Telemetry

Spans open at layer boundaries and are named `bacnet.<layer>.<operation>` —
`bacnet.tag.decode`, `bacnet.npdu.route`, `bacnet.tsm.transaction`. Metrics
carry the device instance and object, so a slow point is findable rather than
merely visible in an average.

Attributes are declared beside the field they describe:

```rust
trace_attrs! { ReadProperty => { object_id: "bacnet.object.id",
                                 property:  "bacnet.property.identifier" } }
```

**An undeclared field cannot be recorded.** There is no `set_attribute` call
for a well-meaning change to add in a hot path, so a value read from equipment
cannot leak into a trace by accident. The core has no telemetry dependency at
all — the traits are inert, and an application that configures nothing pays one
no-op call.

## Conformance

```sh
$ just conformance
  ok   size: 42 items, clean       ok   purity: 4 items, clean
  ok   licence: 40 items, clean    ok   deps: 33 items, clean
  ok   arch: 10 items, clean
```

Two properties of that output are load-bearing. Each gate reports **what it
examined**, and **fails when it examined nothing** — a gate reporting success
because it could not find the tree it was meant to inspect is worse than no
gate, because it is believed. And every gate has a negative test that plants a
violation and requires the gate to catch it: a gate exercised only against a
correct repository reports success both when it works and when it does nothing.

The rules: no hand-written file over 200 lines, documentation included; no I/O
or clock reads in the pure core; dependencies only ever pointing inward; no
OpenTelemetry dependency outside the one adapter crate; and **nothing in the
resolved graph that links a C library**, which is what keeps
`cargo check --target armv7-unknown-linux-gnueabihf` one flag rather than a
cross-compilation project. CI builds every target on every run.

## State

Phase 0 is complete: the architecture, its gates, their negative tests, and CI.

Each phase ends with a committed corpus of hex-dumped traffic that decodes and
re-encodes byte for byte, malformed records included, and no phase starts
before the previous corpus passes. Ahead: the tag codec and primitive types
(3), APDU and NPDU (4), the transaction state machine (5), `ReadProperty` and
`Who-Is` (6), BACnet/IP (7), the first live read end to end (8), then the
breadth of types, services and the object model, MS/TP, BACnet/SC and IPv6.

## Getting started

```sh
just              # every recipe
just conformance  # the gates, and the tests that prove they bite
just test
just ci           # everything CI runs, in CI's order
```

Conventions are in [CLAUDE.md](CLAUDE.md), and they are enforced, not advisory.

## Licence

Apache-2.0.
