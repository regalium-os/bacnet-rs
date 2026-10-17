# bacnet-rs

A pure-Rust implementation of **BACnet** — ASHRAE 135 / ISO 16484-5 — the
building automation protocol that HVAC plant, lighting, metering, access
control and lift equipment speak to each other.

It is a library and a runtime, not an application: no global state, no hidden
threads, and no clock the core reads behind your back. It is built to be
imported by software that must keep talking to lift controllers for years.

> **Status.** Past Phase 8. The driver discovers devices, routed ones
> included, and reads, writes and watches them over BACnet/IP: `ReadProperty`,
> `ReadPropertyMultiple` with fallback, `WriteProperty`, and COV renewed
> before it lapses with polling where it is refused — each service against a
> committed corpus, each value into the protobuf model. The **field probe**
> frames, transacts and speaks every service through the driver.

## Features

| Area | Covers | State |
| --- | --- | --- |
| Core | tags, the 13 primitive types, APDU, NPDU, the client transaction state machine, Who-Is / I-Am / `ReadProperty` (**built**); the other services, object model — sans-io, lossless | Phases 3–6 |
| BACnet/IP | Annex J: BVLL, BBMD, foreign device registration, the UDP link (**built**) | Phase 7 |
| Runtime | discovery, binding through routers, reads, writes, RPM fallback, COV lifecycle, polling, alarms, rebinding with backoff, a served local device with segmentation (**built**) | Phase 8 |
| Schema | `protobuf/` per Google AIP; prost, FlatBuffers and Cap'n Proto; conversion from the decoded services (**built**) | Phase 9 |
| MS/TP | Clause 9 master node over RS-485 | planned |
| BACnet/SC | Annex AB, WebSocket and TLS, hub and direct | planned |
| **BACnet/IPv6** | Annex U: multicast, virtual MAC addresses, BBMD — **[the full list](docs/ipv6.md)** | planned |
| Telemetry | metrics and spans through one seam, OTLP export; `trace_attrs!` | **built** |
| Field probe | discover, browse, watch, write, create, delete, export, dissect; serves the API; TUI | **built** |
| Gates | size, licence, purity, layering, dependencies — each proven to bite | **built** |

Every crate's features, and the order they come in, are in
**[docs/roadmap.md](docs/roadmap.md)**.

## Why another BACnet stack

BACnet stacks tend to fuse the codec to a socket and a clock. That makes the
interesting failures — a retry storm, a segmented response that never
completes, a device that goes quiet mid-transaction — reproducible only
against hardware, which means they are not reproduced at all.

**The core is sans-io.** It is handed octets and a timestamp and returns what
should happen next:

```rust
match tsm.poll(clock.now()) {
    Step::Transmit { peer, apdu, .. } => port.send(peer, &apdu)?,
    Step::Complete { invoke, reply, .. } => answered(invoke, reply),
    Step::Abandon { peer, reason, .. } => went_quiet(peer, reason),
    Step::Idle { wake_at } => clock.park_until(wake_at),
}
```

Nothing there opens a socket or reads a clock, so a full timeout, retry,
segmentation and abandon sequence is a table test on a fake clock. The
runtime is the only thing that waits.

**Decoding is lossless.** Decode then re-encode reproduces the input byte for
byte, *including* a malformed length or a reserved bit. Equipment that does
not follow the standard is the normal case, and a stack that quietly
normalises what a controller sent hides the reason a site does not work.

## A first look

This runs today (`cargo run -p regalium-bacnet --example read`):

```rust
use regalium_bacnet::Client;
use regalium_bacnet::ip::BacnetIp;
use regalium_bacnet::primitive::ObjectIdentifier;
use regalium_bacnet::runtime::Config;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let link = BacnetIp::bind("0.0.0.0:47808".parse()?)?;
    let mut client = Client::new(link, Config::default());

    // Who-Is / I-Am: the exchange every BACnet network runs on.
    for device in client.who_is(None, Duration::from_secs(3))? {
        println!("{} at {}", device.instance, device.address);
    }

    let ai1 = ObjectIdentifier { object_type: 0, instance: 1 };
    let reading = client.read_property(599, ai1, 85)?; // present-value
    println!("{:?}", reading.primitive());
    println!("{}", reading.property()?.name); // the protobuf resource
    Ok(())
}
```

Batches, events, driving the core without the runtime, and what is still
planned — COV, `ReadPropertyMultiple`, serving — are in
**[docs/usage.md](docs/usage.md)**.

## Architecture

The workspace is the layering. Each crate may depend only on the crates above
it, and `cargo xtask arch` reads the resolved dependency graph to prove it — a
module boundary is a convention, a crate boundary is a fact the compiler
already knows.

| Crate | Owns | Pure |
| --- | --- | --- |
| `regalium-bacnet-telemetry` | meter, tracer and logger traits; no dependencies at all | ✅ |
| `regalium-bacnet-core` | tags, types, APDU, NPDU, services, object model, transaction state machine | ✅ |
| `regalium-bacnet-ip` | BACnet/IP — Annex J: BVLL, BBMD, foreign device registration | ✅ |
| `regalium-bacnet-mstp` | MS/TP master — Clause 9, over RS-485 | |
| `regalium-bacnet-sc` | BACnet Secure Connect — Annex AB, over WebSocket and TLS | |
| `regalium-bacnet-ipv6` | BACnet/IPv6 — Annex U: multicast, virtual MAC table | |
| `regalium-bacnet-schema` | generated domain models and their mirrors | |
| `regalium-bacnet-runtime` | the clock, sockets, discovery, polling, subscription lifecycle | |
| `regalium-bacnet` | the public surface — `pub use`, never a newtype | |
| `regalium-bacnet-telemetry-otel` | the only crate that may name the telemetry SDK | |

**Pure** means no I/O and no clock. A re-export is the same type, so
implementing a datalink for a proprietary bridge, or adding an object type,
needs no change here; a wrapper type would compile and then silently close
every extension point. Vendor differences live only in profiles over one
codec — never in a codec of their own.

## Telemetry

Spans open at layer boundaries and are named `bacnet.<layer>.<operation>` —
`bacnet.tag.decode`, `bacnet.npdu.route`, `bacnet.ipv6.vmac.resolve`. Metrics
carry the device instance and object, so a slow point is findable rather than
merely visible in an average. Attributes are declared beside the field they
describe, and **an undeclared field cannot be recorded**: there is no
`set_attribute` for a well-meaning change to add in a hot path, so a value
read from equipment cannot leak into a trace by accident. Export goes through
the the-protobuf-project telemetry SDK; nothing depends on OpenTelemetry
directly, and an application that configures nothing pays one no-op call.

## Conformance

```sh
$ just conformance
  ok   size: 151 items, clean      ok   licence: 145 items, clean
  ok   arch: 11 items, clean       ok   purity: 7 items, clean
  ok   deps: 3080 items, clean
```

Each gate reports **what it examined** and **fails when it examined
nothing**, and every gate has a negative test that plants a violation and
requires the gate to catch it. The rules: no hand-written file over 200 lines,
documentation included; no I/O or clock reads in the pure core; dependencies
only pointing inward; no OpenTelemetry outside the adapter; and **nothing
beneath the driver that links a C library**, which keeps
`cargo check --target armv7-unknown-linux-gnueabihf` one flag rather than a
project. The one audited exception — the telemetry SDK's zstd and lz4 — is
accepted beneath the otel adapter and the probe only.

## Field probe

[`tools/bacnet-probe`](tools/bacnet-probe) discovers, browses, watches,
changes, exports and dissects BACnet/IP devices today, as a TUI and a
scriptable CLI, and serves the schema's device, object and property API over
its own client:

```sh
brew install regalium-os/tap/bacnet-probe
bacnet-probe                                  # TUI: browse, packets, log, settings
bacnet-probe --scan '192.168.*' whois         # sweep every /24 in a /16
bacnet-probe --bbmd 10.1.0.1 read 599 analog-input:1
```

Its **Settings** tab (`s`) edits the target, port, local socket, scan ranges,
BBMD and timing while it runs, checks them as the command line would, and
reopens the socket — keeping the old settings if the new ones cannot be
opened. IPv6 addresses are refused by name until Annex U lands. The probe
carries a small codec of its own until Phase 8, then moves onto the driver.

## Documentation

| | |
| --- | --- |
| [docs/usage.md](docs/usage.md) | the client and the local device: what runs, and what is planned |
| [docs/core.md](docs/core.md) | driving the sans-io core yourself, and choosing a datalink |
| [docs/roadmap.md](docs/roadmap.md) | every crate's features, built and planned, and the phase order |
| [docs/ipv6.md](docs/ipv6.md) | BACnet/IPv6: what Annex U requires and when it is done |
| [tools/bacnet-probe](tools/bacnet-probe/README.md) | the probe: commands, TUI, settings, metrics |
| [CLAUDE.md](CLAUDE.md) | the conventions, which are enforced, not advisory |

## Getting started

```sh
just              # every recipe
just conformance  # the gates, and the tests that prove they bite
just test         # cargo test --workspace --all-targets
just probe        # install bacnet-probe from this checkout
just ci           # everything CI runs, in CI's order
```

## Licence

Apache-2.0.
