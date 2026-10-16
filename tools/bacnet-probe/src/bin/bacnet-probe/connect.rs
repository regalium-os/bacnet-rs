// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Opening what the network options describe: bind the socket, register
//! with a BBMD, plan discovery. The subcommands open it once; the TUI opens
//! it again each time its settings tab applies a change, so both go through
//! here and cannot come to disagree about what `--bbmd` or `--port` means.

use crate::args::Net;
use crate::cli;
use bacnet_probe::{Client, Discovery};
use regalium_bacnet_telemetry::Meter;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

/// A bound, registered client and its Who-Is plan, with what a person should
/// be told about how it was opened: a fallback port, a registration.
pub(crate) struct Opened {
    pub(crate) client: Client,
    pub(crate) plan: Discovery,
    pub(crate) notes: Vec<String>,
}

/// Opens `net` again. Called on the TUI's worker thread, so it is `Send`;
/// it reports by return value because stderr is underneath the TUI.
pub(crate) type Connector = Box<dyn Fn(&Net) -> Result<Opened, String> + Send>;

/// Opens a client for the TUI. The plan is checked before the socket is
/// bound, so a sweep refused as too large never costs the user their port.
///
/// # Errors
///
/// When the plan is refused, the socket cannot be bound, or the BBMD does
/// not accept the registration.
pub(crate) fn for_tui(net: &Net, meter: Arc<dyn Meter>) -> Result<Opened, String> {
    let plan = cli::plan(net).map_err(|e| e.to_string())?;
    let mut notes = Vec::new();
    // Short timeouts: in a UI, a dead device should cost a second, not six.
    let mut client = bind(net, &mut notes)
        .map_err(|e| format!("cannot bind: {e}"))?
        .with_timeout(net.timeout.unwrap_or(Duration::from_secs(1)))
        .with_retries(1)
        .with_meter(meter);
    notes.extend(register(&mut client, net)?);
    if let Ok(local) = client.local_addr() {
        notes.push(format!("listening on {local}"));
    }
    Ok(Opened {
        client,
        plan,
        notes,
    })
}

/// A [`Connector`] for the TUI that records through `meter`.
pub(crate) fn tui_connector(meter: Arc<dyn Meter>) -> Connector {
    Box::new(move |net| for_tui(net, Arc::clone(&meter)))
}

/// Registers with `--bbmd`, if given, before anything is sent, and says so.
///
/// # Errors
///
/// When the BBMD does not answer or refuses.
pub(crate) fn register(client: &mut Client, net: &Net) -> Result<Option<String>, String> {
    let Some(bbmd) = net.bbmd else {
        return Ok(None);
    };
    client
        .register_foreign_device(bbmd, net.ttl)
        .map_err(|e| format!("--bbmd {bbmd}: {e}"))?;
    Ok(Some(format!(
        "registered as a foreign device with {bbmd} for {} s",
        net.ttl
    )))
}

/// Binds `--bind`, or else `0.0.0.0:<port>`, falling back to an ephemeral
/// port with a note when that one is taken.
///
/// # Errors
///
/// When `--bind` cannot be bound, or no port at all can be.
pub(crate) fn bind(net: &Net, notes: &mut Vec<String>) -> io::Result<Client> {
    if let Some(addr) = net.bind {
        return Client::bind(addr);
    }
    // Many devices broadcast I-Am to port 47808, so prefer to own it.
    Client::bind(SocketAddr::from(([0, 0, 0, 0], net.port))).or_else(|e| {
        notes.push(format!(
            "cannot bind :{} ({e}); using an ephemeral port, so broadcast I-Ams may be missed",
            net.port
        ));
        Client::bind(SocketAddr::from(([0, 0, 0, 0], 0)))
    })
}

/// What discovery asks, for a header: the target, or the swept ranges.
pub(crate) fn label(net: &Net) -> String {
    if net.scan.is_empty() {
        net.target.to_string()
    } else {
        cli::scopes(&net.scan)
    }
}
