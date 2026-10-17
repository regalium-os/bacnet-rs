# The core, and datalinks

The runtime is a convenience over two layers anyone can use directly: the
sans-io core, and the datalink trait each network implements. This is how
to drive them yourself.

## Driving the core directly

The runtime is optional. The core is sans-io: hand it octets and a timestamp
and it returns what should happen next, which is what makes a timeout, retry
and abandon sequence a table test rather than a hardware appointment.

```rust
use regalium_bacnet_core::tsm::{Config, Request, Step, Tsm};

let mut tsm = Tsm::new(Config::default());
let request = Request::new(READ_PROPERTY, body).remote(peer_limits);
let invoke = tsm.request(peer, request, clock.now())?;

loop {
    match tsm.poll(clock.now()) {
        Step::Transmit { peer, apdu, .. } => port.send(peer, &apdu)?,
        Step::Complete { reply, .. } => break Ok(reply),
        Step::Abandon { reason, .. } => break Err(reason),
        Step::Idle { wake_at } => match port.recv_until(wake_at)? {
            Some((from, octets)) => {
                tsm.receive(from, &Apdu::decode(&octets)?, clock.now());
            }
            None => {} // the deadline passed; the next poll retries
        },
    }
}
```

Decoding is lossless, so a malformed frame can be inspected rather than
silently normalised:

```rust
let npdu = Npdu::decode(&bvlc.payload)?;
if let Body::Apdu(octets) = npdu.body {
    let apdu = Apdu::decode(octets)?;
    let mut again = Vec::new();
    apdu.encode(&mut again);
    assert_eq!(again, octets);
}
```

## Datalinks

`Client::new` takes any datalink; BACnet/IP is built. The others are
constructed the same way when they land:

```rust
let client = Client::new(MsTp::open("/dev/ttyUSB0", Baud::B38400, MacAddress(3))?, Config::default());
```

BACnet/IPv6 is constructed the same way, with an interface to join the
multicast group on and a virtual MAC address; what Annex U asks of it that
Annex J does not is in [ipv6.md](ipv6.md).

A datalink is a trait the core declares — send an NPDU, wait a bounded time
for one — and each crate satisfies it, so a proprietary bridge is implemented
outside this repository without changing it.
