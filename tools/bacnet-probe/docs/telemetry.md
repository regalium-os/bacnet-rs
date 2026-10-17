# Telemetry: metrics, traces and logs

Everything the probe does can go to an OpenTelemetry collector over OTLP:
`--otlp HOST:PORT` from any command, or `export` to run headless. Metrics,
traces and logs share one set of labels, so a question asked of one can be
asked of the others.

## Labels: who a device is, on everything

Every series, span and log line about a device carries what the device says
about itself, read from its Device object:

| Label | From | For |
| --- | --- | --- |
| `bacnet.device.instance` | the instance | one device |
| `bacnet.device.name` | object-name | what an operator calls it |
| `bacnet.vendor.name`, `bacnet.vendor.id` | vendor-name, vendor-identifier | every device from one vendor |
| `bacnet.device.model` | model-name | every device of one model |
| `bacnet.device.firmware` | firmware-revision | every device on firmware 11 |
| `bacnet.device.software` | application-software-version | the application loaded |
| `bacnet.device.protocol_revision` | protocol-revision | revision 12 against revision 19 |
| `bacnet.device.location` | location | one plant room |

Series about an object add `bacnet.object.type`, `.instance`, `.name` and
`bacnet.object.kind` (`analog`, `binary`, `multi-state`). These are per
device, not per reading, so they cost a label value per device, never a
series per value. A value read from equipment is never a label.

```promql
bacnet_point_value{bacnet_device_firmware="11.2"}               # every point on 11.2
count by (bacnet_device_model) (bacnet_device_up == 0)          # what is offline, by model
```

## Metrics

| Instrument | Kind | Labels |
| --- | --- | --- |
| `bacnet.point.value` | gauge | the device's identity, the object's labels, units |
| `bacnet.point.status` | gauge, 0/1 | the point's labels, and `bacnet.status.flag` (in-alarm, fault, overridden, out-of-service) |
| `bacnet.point.event_state`, `bacnet.point.reliability` | gauge (code) | the point's labels |
| `bacnet.point.reachable` | gauge, 0/1 | the point's labels |
| `bacnet.device.up` | gauge, 0/1 | the device's identity, `server.address`, network |
| `bacnet.device.objects` | gauge | the device's identity |
| `bacnet.discovery.devices` | gauge | — |
| `bacnet.client.requests` | counter | service, outcome, device instance |
| `bacnet.client.request.duration` | histogram, ms | service, device instance |
| `bacnet.client.packets`, `bacnet.client.bytes` | counter | direction |
| `bacnet.cov.notifications` | counter | device instance |

Binary points export 0/1, multi-state points their state number. In
Prometheus the names become `bacnet_point_value` and so on.
`OTEL_METRIC_EXPORT_INTERVAL` (ms) sets how often they are sent.

## Traces

A trace nests the way the work did. A scenario's span contains the API calls
it made, which contain the BACnet requests those made:

```
bacnet.e2e.cycle
  bacnet.e2e.write
    bacnet.api.UpdateProperty
      bacnet.client.writeProperty   analog-output 1 present-value  priority 8
      bacnet.api.GetProperty
        bacnet.client.readProperty  analog-output 1 present-value
```

Request spans record the object and property, the invoke id, how many
attempts it took, and the outcome; a refusal records `bacnet.error.class` and
`bacnet.error.code` and ends the span as an error. Discovery spans record the
datagrams sent and the devices found.

## Logs

Lines go to OTLP only, never the console, at `debug` (every request and
reply), `info` (what happened, at reading pace), `warn` (a refusal, a
timeout) and `error` (a value that did not read back as written). Each line
carries `trace_id` and `span_id`, so a log query leads to its trace, and the
device's identity, so it filters like the metrics do.

## End to end, with mock data

```sh
cargo run -p regalium-bacnet-probe --example e2e -- localhost:6009 10 my-run
```

Starts a simulated gateway whose values move, and runs ten cycles against
it, each one trace: discover, poll every point into metrics, write a
sine-wave setpoint at priority 8 and read it back, override an input and
restore it, provoke a refusal the device must give, create and delete an
object, and follow COV. Filter by `run="my-run"` to see only that run.

## Dashboard

[`dist/grafana/bacnet.json`](../dist/grafana/bacnet.json) is a Grafana
dashboard over all of the above. `just grafana` imports it into the local
stack; `just grafana URL USER:PASSWORD` into another. Its datasources are
variables, so it works against any Prometheus, Loki and Tempo.

| Section | What it shows | The TUI's |
| --- | --- | --- |
| Overview | devices up and down, points, alarms, faults, overrides, unreachable points, request rate, error share, p95 latency, COV rate | header and footer |
| Devices | every device with its identity, address and object count; availability over time | Devices pane |
| Points | every point's value, status flags, event state and reliability; analog trends, one panel per unit; binary and multi-state as state timelines | Objects pane |
| Requests and packets | requests by service and outcome, latency percentiles and heatmap, packets and bytes by direction, COV notifications | Packets tab |
| Traces | recent and failed traces, refused requests, the slowest requests, and any one trace in full | — |
| Logs | volume by level, every line, warnings and errors | Log tab |

The filters at the top — service, vendor, model, firmware, device, instance,
point kind and log level — narrow every panel at once. To open a trace, copy
its ID from a table, or a log line's `trace_id`, into the Trace ID box.
