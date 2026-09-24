# CLAUDE.md

Conventions for working in this repository. These are enforced by `xtask` and
by CI, not left to reviewer memory.

`bacnet-rs` is a pure-Rust implementation of ASHRAE 135 / ISO 16484-5. It is a
**library and a runtime**, not an application: it is imported by building
software that must keep talking to lift controllers for years. Design for that
caller. Its sibling is `osdp-go`, and where this document is silent, that
repository's CLAUDE.md is the precedent.

## Non-negotiables

Each of these fails CI. Run `just conformance` before you think you are done.

| Rule | Enforced by |
| --- | --- |
| No hand-written file exceeds 200 lines, **documentation included** | `xtask size` |
| **No direct OpenTelemetry dependency**, outside the adapter | `xtask deps` |
| **Nothing in the graph links a C library** | `xtask deps` + CI `cross` |
| The pure core performs no I/O and reads no clock | `xtask purity` |
| Dependencies only ever point inward | `xtask arch` |
| Every hand-written file carries the Apache-2.0 notice | `xtask licence` |
| Every gate fails when it examined nothing | `xtask::scan::Findings::verdict` |
| Every gate has a test proving it bites | `xtask/tests/` |
| Clean under `clippy -D warnings` and `cargo fmt --check` | CI `lint` |
| Licences and advisories across the whole graph | `deny.toml`, CI `deny` |
| `.proto` satisfies Google AIP, strictly | `.github/workflows/api-lint.yml` |

## The 200-line limit

The limit is about comprehension. A file that fits in two screens can be held
in the head while reading it, and a protocol codec nobody can hold in their
head is a codec where the off-by-one lives.

When a file approaches the ceiling, split it along a seam that already exists:
encode from decode, one service family from another, the state machine from its
transition table. Name the halves for what they contain.

**Never reclaim lines by deleting documentation.** If the choice is a 210-line
file with its comments or a 190-line file without them, the file was already
doing two jobs — split it. The limit applies to Markdown for the same reason a
reference nobody finishes is as useless as a function nobody can follow.

Generated code under `**/generated/` is exempt. Test files are not.

## Licensing

Apache-2.0. Every hand-written file opens with the notice and its SPDX
identifier, in that file kind's comment syntax, followed by a blank line:

```rust
// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0
```

`xtask licence` is the authority on which kinds carry one; add a kind there
rather than working around it. It reads only the first four lines, because an
identifier buried halfway down is not a notice.

## Documentation

Every crate's `lib.rs` opens with a `//!` **layer contract**: what it owns,
what it must not do, which crates it may import, which spans it opens. Read the
existing ones before adding a crate; match their shape.

Every public item has a doc comment. For a protocol library that means:

- **Cite the specification.** `/// Per ASHRAE 135-2016 §20.2.1.3, a context tag
  of 5 with length 5 is the extended-length escape; the octet that follows is
  the real length.` A reader fixing a bug at 3am needs the clause.
- **Say why, not what.** That a hop count starts at 255 is self-evident from the
  code; *why the standard chose a hop count rather than a time-to-live* is not.
- **Document whether a decode borrows or copies.** Zero-copy decoding that
  borrows the input buffer is a real design decision with real lifetime
  consequences for the caller. State it on every function taking `&[u8]`.
- **Document concurrency** on every public type: `Send`/`Sync` is what the
  compiler says, but whether concurrent use is *correct* is what you say.
- **Record real-world quirks with a fixture.** A comment saying a vendor
  misreports a property must point at the fixture that proves it.

## Library API

- Constructors take required arguments positionally and optional ones as
  builder methods. Adding an option must never break a caller.
- Return typed errors carrying the offending bytes, matched with pattern
  matching rather than string comparison. A caller distinguishing "bad tag" from
  "short read" must not resort to matching on a message.
- Accept traits, return concrete types. A trait is declared by the consumer at
  the boundary — which is why the datalink trait lives in the core and each
  datalink merely satisfies it.
- Make `Default` meaningful where it costs nothing, and say so.
- **Never panic.** Not on malformed input, not on a short buffer. No indexing,
  no `unwrap`, no `expect` outside tests. Malformed input is the normal case on
  a wire shared with noise, and a driver that aborts takes the building with it.

## Runtime API

- Lifecycle is explicit. `new` constructs, `run` blocks until cancelled or
  fatally failed. **`new` never spawns a task.** A caller must be able to
  construct, inspect and discard.
- If a component spawns tasks, it owns their shutdown, and `run` does not
  return until they have stopped. No orphans.
- `close` is idempotent and safe after a failed `run`.
- Time is injected. The core never reads a clock; it returns a decision and a
  deadline, and the runtime acts on it. This is what lets a full
  discovery/timeout/retry/resubscribe scenario run as a table test with a fake
  clock and no hardware.
- Channel direction is part of the signature, and the doc comment says who
  closes it and what happens when the reader is slow. Backpressure is a design
  decision, never an accident.
- A device going offline is an expected event, not an error return. Report it
  through the event stream; reserve errors for what the caller must fix.

## Layering

The table in `xtask/src/arch.rs` is the authority. Change it there,
deliberately, and say why in the commit — never by routing an import around it.

`regalium-bacnet-telemetry` is importable everywhere. The facade re-exports
with `pub use` and never a newtype: a re-export is the same type, so a third
party can implement a datalink or add an object type without this repository
being involved. A wrapper would compile and then silently close every
extension point.

Vendor differences live **only** in a profile: the revision-12 and revision-19
Schindler object models are two profiles over one codec. A vendor never
reimplements framing. If one appears to need its own codec, that is a quirk
flag in the codec with a fixture proving it.

## Telemetry

**Never depend on `opentelemetry`.** Not in the core, not in a datalink, not in
a test, not "just for the attribute type". Observability goes through the
the-protobuf-project telemetry SDK, and `regalium-bacnet-telemetry-otel` is the
only crate that may name it. Two things deciding how a span is made is how
instrumentation rots.

Spans open at layer boundaries and are named `bacnet.<layer>.<operation>`.
Attributes are declared beside the field they describe with `trace_attrs!`, and
**an undeclared field cannot be recorded** — that is a safety mechanism, not an
oversight. Record a property's identifier and type, never a credential, a key,
a session secret, or a nonce.

## Schemas

`protobuf/` is the source of truth: resource-oriented per Google AIP, standard
methods, hierarchical names like `devices/{device}/objects/{object}`. House
style is `{device}`, not `{device_id}`.

The FlatBuffers and Cap'n Proto mirrors are generated by `buffers` from the
same descriptor set — never hand-written. `buffers.lock` records the target
slot every field was assigned, and a moved slot is a wire break: a consumer
compiled against ordinal 5 reads ordinal 5 forever, and nothing in the chain
reports it when a rebuild puts something else there.

## Tests and phase gates

Every phase ends with a fixture corpus: hex-dumped BACnet traffic decoded byte
for byte. **Do not start a phase before the previous corpus passes.**

Decoding must be lossless: decode then re-encode reproduces the input exactly,
including a malformed length, so traffic from equipment that does not follow
the standard can be inspected rather than silently normalised.

A test that cannot see what it is meant to check must **fail**, not pass. Every
corpus walker asserts it found files, every gate asserts it examined something,
and every gate has a negative test. A gate reporting success because it could
not find what it was meant to inspect is worse than no gate, because it is
believed.

Test names are sentences: `a_notice_below_the_header_does_not_count`.

## Commands

```sh
just conformance  # the gates, and the tests that prove they bite -- run this
just test         # cargo test --workspace --all-targets
just fixtures     # the phase gate
just lint         # fmt and clippy, as CI runs them
just deny         # licences and advisories
just cross        # every target CI builds
just ci           # everything above, in CI's order
```

## Do not

- Add a dependency to a pure crate. The core's only dependency is the telemetry
  seam, and the seam has none. That is the target state, not an accident.
- Depend on OpenTelemetry. See Telemetry above.
- Weaken a test or a gate to make it pass. Fix the code, or change the rule in
  `xtask` deliberately and say why in the commit.
- Reimplement the codec for a vendor. Add a profile, or a quirk flag with a
  fixture.
- Commit or push unless asked.
