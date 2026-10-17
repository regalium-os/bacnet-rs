# Using bacnet-rs

What runs today is marked **built**; the rest is the API being built to,
written down so its shape can be argued with. The client blocks: each call
waits for its answer or gives up, which needs no async runtime. An async
surface will wrap it rather than replace it.

## Discovery — built

Every BACnet network runs on `Who-Is` / `I-Am`. A device answers with its
instance number, the maximum APDU it can receive, and whether it segments.

```rust
use regalium_bacnet::Client;
use regalium_bacnet::ip::BacnetIp;
use regalium_bacnet::runtime::Config;

let link = BacnetIp::bind("0.0.0.0:47808".parse()?)?;
let mut client = Client::new(link, Config::default());

for device in client.who_is(None, Duration::from_secs(3))? {
    println!("{} at {} {:?}", device.instance, device.address, device.route);
}
```

Narrow it when you know what you want; `who_is_at` sends to one address
when broadcast is not an option; `bind_device` returns as soon as its device
answers. Devices behind a router are bound with their network, and every
request to them is routed:

```rust
let plant = client.who_is(Some((500, 599)), Duration::from_secs(2))?;
let gateway = client.bind_device(599, Duration::from_secs(2))?;
```

## Reading — built

One property, as a value, or as the protobuf model's `Property` with its
octets kept:

```rust
let ai1 = ObjectIdentifier { object_type: 0, instance: 1 };
let reading = client.read_property(599, ai1, 85)?;
let value = reading.primitive();          // Some(Primitive::Real(21.5))
let model = reading.property()?;          // devices/599/objects/analog-input-1/...
```

Many at once: `read_many` keeps several in flight and returns each result in
the order asked, each failure in its place:

```rust
let asks = [ReadRequest::new(599, ai1, 85), ReadRequest::new(10, ai1, 85)];
for result in client.read_many(&asks)? {
    match result {
        Ok(r) => println!("{:?}", r.primitive()),
        Err(ReadError::Offline(why)) => println!("no answer: {why}"),
        Err(e) => println!("{e}"),
    }
}
```

A device that stops answering is an event, reported once, and reported
again when it comes back — not an error to handle on every read:

```rust
for event in client.events() {
    // Discovered(599), Offline { instance: 599, .. }, Reachable(599)
}
```

`cargo run -p regalium-bacnet --example read -- ADDRESS DEVICE OBJECT
PROPERTY` does all of this against a real device.

## Reading many in one request — planned

`ReadPropertyMultiple` is one round trip rather than one per point, which is
the difference between polling a thousand points a second and a minute:

```rust
let readings = client
    .read_property_multiple(device)
    .object((ObjectType::AnalogInput, 1))
        .properties([PropertyId::PresentValue, PropertyId::Units, PropertyId::StatusFlags])
    .send()?;
```

Not every device implements `ReadPropertyMultiple`, and the standard permits
that. A device that rejects it is a capability to record once, not an error to
retry — the runtime falls back to `ReadProperty` and remembers.

## Subscribing — planned

Polling a point that changes twice a day is waste. Change-of-value pushes
instead, and the lifetime a subscription carries is renewed for you before it
lapses:

```rust
let mut changes = client
    .subscribe_cov(device, (ObjectType::AnalogInput, 1))
    .lifetime(Duration::from_secs(300))
    .confirmed(true)
    .await?;

while let Some(change) = changes.next().await {
    println!("{} = {}", change.property(), change.value());
}
```

`confirmed(true)` asks the device to expect an acknowledgement for each
notification, so a dropped datagram is detectable. It requires a local device
to acknowledge from, which the client will create.

`subscribe_cov_property` narrows a subscription to one property and can set an
increment, so a noisy analogue reports on real movement rather than on its own
last digit.

## Serving — planned

A local device answers `Who-Is`, owns an object database, and can be the
recipient of event and alarm notifications:

```rust
let device = LocalDevice::new(599)
    .name("plant-gateway")
    .object(AnalogValue::new(1).present_value(21.5).units(Units::DegreesCelsius))
    .object(BinaryValue::new(1).present_value(false))
    .serve(BacnetIp::bind("0.0.0.0:47808").await?)
    .await?;
```

## Driving the core directly — built

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
