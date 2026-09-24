# Using bacnet-rs

The API this library is being built to. Nothing here runs yet — see
[State](../README.md#state) — but the shape is settled enough to argue with.

## Discovery

Every BACnet network runs on `Who-Is` / `I-Am`. A device answers with its
instance number, the maximum APDU it can receive, and whether it segments.

```rust
use regalium_bacnet::{Client, ObjectType, PropertyId};
use std::time::Duration;

let client = Client::bind("0.0.0.0:47808").await?;

let devices = client.who_is().within(Duration::from_secs(3)).collect().await?;
for device in &devices {
    println!("{} — {} at {}", device.instance(), device.name(), device.address());
}
```

Narrow the broadcast when you know what you are looking for, and bind a single
device by instance:

```rust
let plant = client.who_is().range(500..=599).within(Duration::from_secs(2)).collect().await?;
let gateway = client.bind_device(599).await?;
```

`Who-Has` / `I-Have` does the same for an object rather than a device, which is
how you find a point whose device you do not know.

## Reading

One property:

```rust
let temperature: f32 = client
    .read_property(device, (ObjectType::AnalogInput, 1), PropertyId::PresentValue)
    .await?
    .try_into()?;
```

Many at once. `ReadPropertyMultiple` is one round trip rather than one per
point, which is the difference between polling a thousand points a second and
a minute:

```rust
let readings = client
    .read_property_multiple(device)
    .object((ObjectType::AnalogInput, 1))
        .properties([PropertyId::PresentValue, PropertyId::Units, PropertyId::StatusFlags])
    .object((ObjectType::BinaryInput, 3))
        .properties([PropertyId::PresentValue])
    .send()
    .await?;
```

A property that is an array — an object list, say — reads by index, or whole:

```rust
let count: u32 = client
    .read_property(device, device.object_id(), PropertyId::ObjectList)
    .index(0)
    .await?
    .try_into()?;
```

Not every device implements `ReadPropertyMultiple`, and the standard permits
that. A device that rejects it is a capability to record once, not an error to
retry — the runtime falls back to `ReadProperty` and remembers.

## Subscribing

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
to acknowledge from, which `Client::bind` creates.

`subscribe_cov_property` narrows a subscription to one property and can set an
increment, so a noisy analogue reports on real movement rather than on its own
last digit.

## Serving

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

## Driving the core directly

The runtime is optional. The core is sans-io: hand it octets and a timestamp
and it returns what should happen next, which is what makes a timeout, retry
and abandon sequence a table test rather than a hardware appointment.

```rust
use regalium_bacnet_core::tsm::{Step, Tsm};

let mut tsm = Tsm::new(Config::default());
let invoke = tsm.request(request, now)?;

loop {
    match tsm.poll(clock.now()) {
        Step::Transmit { bytes, deadline } => port.send(bytes, deadline)?,
        Step::Retry { attempt, bytes, .. } => port.send(bytes, /* ... */)?,
        Step::Abandon { invoke, reason } => break report_timeout(invoke, reason),
        Step::Idle { wake_at } => clock.park_until(wake_at),
    }
}
```

Decoding is lossless, so a malformed frame can be inspected rather than
silently normalised:

```rust
let (npdu, apdu_bytes) = Npdu::decode(&bvlc.payload)?;
let apdu = Apdu::decode(apdu_bytes)?;

assert_eq!(apdu.encode_to_vec(), apdu_bytes);
```

## Datalinks

`Client::bind` uses BACnet/IP. The others are selected by constructing them:

```rust
let client = Client::over(MsTp::open("/dev/ttyUSB0", Baud::B38400, MacAddress(3))?).await?;
```

A datalink is a trait the core declares and each crate satisfies, so a
proprietary bridge is implemented outside this repository without changing it.
