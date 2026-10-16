# BACnet/IPv6

What `regalium-bacnet-ipv6` is to build: the BACnet/IPv6 datalink of
ASHRAE 135 **Annex U**. Today the crate is a layer contract and nothing else;
no frame is encoded or decoded. This page is the feature list it is held to,
and the definition of done. Where it lands in the order of work is in
[the roadmap](roadmap.md#datalinks).

Until it does, the probe refuses an IPv6 address by name — `--target fe80::1`
is an error that says Annex U is not supported yet — rather than sending
Annex J frames to an IPv6 socket, where they would reach nothing.

## Why it is its own datalink

BACnet/IPv6 is not Annex J with a wider address. Three things differ, and
each one is work:

- **There is no broadcast.** IPv6 has multicast instead, so a local
  broadcast is a send to a multicast group, and a node must join the group to
  hear one. The BBMD still exists, to carry broadcasts between groups that
  multicast routing does not join.
- **The IP address is not the BACnet MAC.** A B/IPv6 address is eighteen
  octets — sixteen of address and two of port — which is too large to sit in
  routing tables and NPDU addresses comfortably. Annex U gives each node a
  three-octet **virtual MAC address (VMAC)** instead, and the datalink
  resolves VMAC to address the way ARP resolves IP to Ethernet.
- **Every frame says who sent it.** The BVLL header carries the source VMAC,
  and a unicast also carries the destination's, so a receiver learns
  mappings from ordinary traffic as well as from resolution.

## Features

### BVLL-IPv6 framing

The header is a type octet (`0x82`, where Annex J is `0x81`), a function
octet and a two-octet length, followed by the function's fields. Each
function below gets an encoder, a lossless decoder and a fixture:

| Code | Function | Carries |
| --- | --- | --- |
| `0x00` | BVLC-Result | a result code, for registration and table operations |
| `0x01` | Original-Unicast-NPDU | source and destination VMAC, NPDU |
| `0x02` | Original-Broadcast-NPDU | source VMAC, NPDU; sent to the multicast group |
| `0x03` | Address-Resolution | who has this VMAC? |
| `0x04` | Forwarded-Address-Resolution | the same, relayed by a BBMD with the original source |
| `0x05` | Address-Resolution-ACK | I do |
| `0x06` | Virtual-Address-Resolution | what is your VMAC? |
| `0x07` | Virtual-Address-Resolution-ACK | this one |
| `0x08` | Forwarded-NPDU | a broadcast relayed between BBMDs, with its origin |
| `0x09` | Register-Foreign-Device | a time-to-live |
| `0x0A` | Delete-Foreign-Device-Table-Entry | the entry |
| `0x0C` | Distribute-Broadcast-To-Network | a foreign device's broadcast, for the BBMD to spread |

These codes are transcribed for planning. The codec's fixtures are the
authority, and each is checked against the 135-2016 text as it lands —
which is also when any function this table is missing gets found.

### The VMAC table

- A node's own VMAC, configured or defaulted. The device instance fits in
  three octets, so it is the natural default; a router port without a device
  object needs one configured.
- A cache of VMAC to B/IPv6 address, filled by resolution and by every
  frame received, with entries that age out.
- **Resolution as decisions, not sleeps.** Like the rest of the stack, the
  table is handed frames and a timestamp and returns what to send and when
  to give up. The runtime does the waiting, so a resolution that times out,
  or answers twice, is a table test on a fake clock.
- **Duplicate detection.** Two addresses claiming one VMAC is a fault on the
  network. It is reported as an event naming both, never silently resolved
  in favour of whichever answered last.

### Multicast

- Join the BACnet multicast group on the configured interface. Annex U's
  well-known groups are `FF02::BAC0` (link-local) and `FF05::BAC0`
  (site-local); the port is 47808, `0xBAC0`, as for Annex J.
- Scope and interface are configuration, not guesses: a host with several
  IPv6 interfaces joins on the one it is told to, and says which.

### BBMD and foreign devices

- **Foreign device registration**, as the probe already does for Annex J:
  register with a BBMD, renew before the time-to-live and its grace period
  lapse, and send broadcasts as Distribute-Broadcast-To-Network.
- **A BBMD**: a broadcast distribution table of peer BBMDs, a foreign device
  table with expiry, Forwarded-NPDU in both directions, and relaying address
  resolution for foreign devices that cannot hear the multicast group.
- Table configuration belongs to the Network Port object, not to BVLL
  messages.

### The object model

The Network Port object's IPv6 properties — address, prefix length, UDP
port, multicast address, mode (normal, foreign, BBMD), and the VMAC table —
live in the core's object model, so a device served by this stack reports
its BACnet/IPv6 port like any other.

### Telemetry

Spans `bacnet.ipv6.{send,receive}` and `bacnet.ipv6.vmac.resolve`, per the
crate's layer contract. Metrics cover resolution outcomes, the cache size,
duplicates seen, and registration state. A VMAC and an address may be
recorded; nothing from inside the NPDU is.

## Target API

The shape this is built to, like the rest of [usage.md](usage.md); nothing
here runs yet:

```rust
let port = BacnetIpv6::bind("[::]:47808", "eth0")
    .vmac(Vmac::from_instance(599))
    .group(Ipv6Multicast::SiteLocal)
    .await?;
let client = Client::over(port).await?;
```

## Done when

- A corpus of hex-dumped Annex U traffic — every function above, and the
  malformed cases (short header, a length that disagrees with the datagram)
  — decodes and re-encodes byte for byte.
- Resolution, ageing and duplicate detection pass as table tests on a fake
  clock, with no socket.
- Two nodes on `::1` discover each other and read a property, in CI.
- A foreign device registers with a BBMD and hears a broadcast relayed
  through it.
- The probe dissects BVLC `0x82` frames, and accepts an IPv6 `--target` once
  it has moved onto the driver.
