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
        Err(RequestError::Offline(why)) => println!("no answer: {why}"),
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

## Reading many in one request — built

`ReadPropertyMultiple` is one round trip rather than one per point, which is
the difference between polling a thousand points a second and a minute. Each
property comes back with its value or its own error:

```rust
let reads = [ReadAccess { object: ai1, properties: vec![PropertyReference::new(85), PropertyReference::new(111)] }];
for result in client.read_multiple(599, &reads)? {
    println!("{:?}", result.map(|r| r.primitive()));
}
```

Not every device implements `ReadPropertyMultiple`, and the standard permits
that. A device that rejects it is a capability to record once, not an error to
retry: the client reads it property by property from then on, and remembers.

## Writing — built

```rust
client.write_property(599, ao1, 85, &Primitive::Real(42.5), Some(8))?;
```

A write at a priority can be acknowledged and change nothing, when a higher
priority holds the value (§19.2); the priority array says which slot wins.

## Watching — built

Polling a point that changes twice a day is waste. A watch subscribes for
change of value where the device allows it — renewing the subscription before
it lapses, acknowledging confirmed notifications — and polls where it refuses,
so every point arrives through one stream:

```rust
let id = client.watch(Watch::new(599, ai1).every(Duration::from_secs(10)))?;
loop {
    client.listen(Duration::from_secs(1))?;
    for update in client.updates() {
        println!("{} via {:?}: {:?}", update.watch, update.source, update.values);
    }
}
```

`Watch::cov(None)` polls only. `unwatch` cancels the subscription.
`SubscribeCOVProperty`, with its increment, is in the core and the model, and
reaches the client with the next round of the runtime.

## Serving — built

A local device answers Who-Is and Who-Has, and serves reads and writes from
a database — `Memory`, or anything implementing `obj::Database`, so points
living in a PLC or another protocol are served the same way:

```rust
let db = Memory::new(599, "plant-gateway", 260)
    .object(av1, "Setpoint")
    .with(av1, 85, &Primitive::Real(21.5))
    .writable(av1, 85)
    .object(ao1, "Fan speed")
    .commandable(ao1, &Primitive::Real(0.0));  // sixteen priority slots
let mut device = Server::new(BacnetIp::bind("0.0.0.0:47808".parse()?)?, db);
device.announce()?;
loop {
    device.serve(Duration::from_secs(1))?;
    device.database_mut().set(ai1, 85, &Primitive::Real(read_sensor()));
}
```

It does not segment yet: an answer too large for the requester is aborted
as segmentation-not-supported, which §5.4.5 allows a device that does not.
Event notifications to the client arrive through `client.alarms()`,
acknowledged when confirmed.

## Underneath

Driving the core without the runtime — the transaction state machine on a
clock of your own, lossless decoding — and choosing a datalink are in
**[core.md](core.md)**.
