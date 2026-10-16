# bacnet-probe

Discover, browse, watch and export BACnet/IP devices from a terminal: an
interactive TUI, and a scriptable CLI whose output is a table or JSON.

```sh
brew install regalium-os/tap/bacnet-probe
```

Or take an archive from the
[releases](https://github.com/regalium-os/bacnet-rs/releases) (macOS arm64 /
x86_64, Linux x86_64 / arm64), or build this checkout with `just probe`.

## Use

```sh
bacnet-probe                                  # TUI: devices, objects, properties, packets
bacnet-probe whois                            # who answers a broadcast Who-Is
bacnet-probe --scan '192.168.*' whois         # sweep every /24 in 192.168.0.0/16
bacnet-probe read 599 analog-input:1          # present-value, with units and state names
bacnet-probe read 599 device:599 vendor-name firmware-revision
bacnet-probe find 'Car 1 load'                # which device holds it (Who-Has)
bacnet-probe watch 599 --every 1              # every change: COV where allowed, else polled
bacnet-probe scan --detail --format json      # every property of every object
bacnet-probe --otlp localhost:4317 export     # run headless; everything as metrics
bacnet-probe dissect '81 0a 00 11 …'          # break a captured datagram down, offline
```

Every command takes `--format table|json`. stdout carries only the result;
progress and `--explain` (a field-by-field dissection of every packet) go to
stderr, so `> scan.json` stays clean.

| Option | |
| --- | --- |
| `--target IP` | where Who-Is goes: a subnet broadcast or one device or router |
| `--scan RANGE` | sweep `192.168.*`, `10.1.0.0/16` or one address, and accept answers only from there; repeatable |
| `--unicast`, `--rate N` | with `--scan`: one datagram per host instead of per /24, at N per second (default 500) |
| `--bbmd IP[:PORT]`, `--ttl S` | register as a foreign device and discover through that BBMD |
| `--port`, `--bind` | UDP port (47808) and local socket |
| `--wait`, `--timeout` | seconds to collect I-Am; seconds per reply before retrying (2, or 1 in the TUI) |
| `--otlp HOST:PORT`, `--label K=V` | export metrics from any command, with resource labels |

Addresses are IPv4. An IPv6 address is refused with a message saying so:
BACnet/IPv6 is a different datalink (Annex U), and the probe gains it when it
moves onto the driver — see [docs/ipv6.md](../../docs/ipv6.md).

## The TUI

`bacnet-probe` with no subcommand opens four tabs; `?` lists every key.

| Tab | |
| --- | --- |
| `1` Browse | devices, their objects, and every property of the selected one, live |
| `2` Packets | every datagram sent and received, dissected field by field |
| `3` Log | discoveries, value changes, points going offline, settings applied |
| `4` Settings | the network options, editable while the probe runs (`s` from anywhere) |

**Settings** lists the target, UDP port, local socket, scan ranges, unicast
sweep and its rate, BBMD and registration TTL, I-Am wait, reply timeout and
poll interval — each beside the flag it mirrors. `Enter` edits one (every key
then belongs to the field, so the digits of an address do not switch tabs),
`Space` toggles, `a` applies, `u` undoes every edit and `d` fills in the
defaults. A value is checked by the same parser as its flag, an IPv6 address
or a unicast sweep too large to send is refused beside the field, and nothing
reaches the wire until it all checks.

Applying cancels the COV subscriptions from the old socket, closes it, opens
the new one, registers with the BBMD if one is set, and discovers again. If
the new settings cannot be opened — a port another process holds, a BBMD that
refuses — the old ones are reopened and the error is shown, so a typo costs
a message rather than the session. The tab also shows the address the socket
is listening on, and the command line that starts the probe with what is on
screen, ready to paste into a script.

### Reaching devices

A broadcast reaches one subnet. To find devices elsewhere:

- `--scan` sends one broadcast per /24 in the range (256 for a /16), which
  works where routers forward directed broadcasts, and with `--unicast` one
  datagram per host where they do not. Answers from outside the range are
  dropped, so a range is also an allow-list.
- A device that answers with a *local* broadcast never reaches a probe on
  another subnet, however it was asked. Register with that subnet's BBMD
  (`--bbmd`): the probe then hears its broadcasts, and asks through it. The
  registration renews itself before its TTL lapses.
- A device behind a router (MS/TP behind a BACnet/IP gateway) is found and
  read through the router; the TUI marks it `↳`.

### Live values

`watch`, `export` and the TUI subscribe to change-of-value (SubscribeCOV)
where the device accepts it, so a change arrives when it happens rather than
at the next poll, and poll only what refuses. Subscriptions are renewed at
three quarters of their lifetime and cancelled when the TUI moves on; one left
behind by Ctrl-C lapses within `--lifetime` (120 s). `--no-cov` polls
everything.

Reads go through ReadPropertyMultiple where the device has it: a controller
is read in a handful of round trips rather than one per property. A device
without it is noticed once and read property by property; a request too big
for the device is halved until it fits.

## Metrics

Everything read becomes a series, labelled by *which* point it is — never by
what it reads. Through an OpenTelemetry collector into Prometheus, for example:

```
bacnet_point_value{bacnet_device_instance="599",bacnet_object_type="analog-input",
  bacnet_object_instance="1",bacnet_object_name="Car 1 load",bacnet_units="kilograms"} 420
```

| Instrument | Kind | Labels |
| --- | --- | --- |
| `bacnet.point.value` | gauge | device instance and name, object type, instance and name, units |
| `bacnet.point.status` | gauge, 0/1 | the point's labels, and `bacnet.status.flag` (in-alarm, fault, overridden, out-of-service) |
| `bacnet.point.event_state`, `bacnet.point.reliability` | gauge (code) | the point's labels |
| `bacnet.point.reachable` | gauge, 0/1 | the point's labels |
| `bacnet.device.up` | gauge, 0/1 | device instance and name, vendor, `server.address`, network |
| `bacnet.device.objects` | gauge | device instance and name |
| `bacnet.discovery.devices` | gauge | — |
| `bacnet.client.requests` | counter | service, outcome, device instance |
| `bacnet.client.request.duration` | histogram, ms | service, device instance |
| `bacnet.client.packets`, `bacnet.client.bytes` | counter | direction |
| `bacnet.cov.notifications` | counter | device instance |

Binary points export 0/1, multi-state points their state number. Values go
through the workspace's telemetry seam and the-protobuf-project telemetry
SDK (`regalium-bacnet-telemetry-otel`): OTLP gRPC to `--otlp`, else to what
`telemetry.toml` or `TELEMETRY_*` say. `OTEL_METRIC_EXPORT_INTERVAL` (ms) sets
how often. `export` rediscovers every `--rescan` seconds (300), so a device
that stops answering reads `bacnet.device.up 0` instead of vanishing.

## Without hardware

```sh
cargo run -p regalium-bacnet-probe --example sim_device          # a lift gateway on :47809
bacnet-probe --target 127.0.0.1 --port 47809 --bind 127.0.0.1:0  # then any command
```

The simulator answers Who-Is, Who-Has, ReadProperty and
ReadPropertyMultiple, sends COV notifications, accepts foreign device
registration, and moves its values over time. The tests run every feature
against it over real UDP.

## Scope

Read-only: the probe never writes a property, so it is safe to point at
equipment in service. It does not reassemble segmented replies; it keeps its
requests small enough not to need them.

It carries a small codec of its own because the driver's core does not decode
a frame until Phase 3. At Phase 8, the driver's first live read, the probe
moves onto `regalium-bacnet` and this codec is deleted.
