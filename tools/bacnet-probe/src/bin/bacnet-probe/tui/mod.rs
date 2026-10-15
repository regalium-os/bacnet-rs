// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `bacnet-probe tui`: browse devices, objects and properties live, and
//! watch every packet go by, dissected.
//!
//! Two threads. The worker owns the socket and does all network I/O; this
//! thread owns the terminal and never blocks on the wire. They talk over two
//! channels: commands down, events up.

mod app;
mod keys;
mod ui;
mod worker;

use app::App;
use bacnet_probe::{Client, Discovery};
use ratatui::crossterm::event::{self, Event as Input};
use std::sync::mpsc;
use std::time::Duration;
use worker::Worker;

/// How often the screen redraws when nothing happens (highlights fade).
const TICK: Duration = Duration::from_millis(100);

/// Runs the TUI until the user quits. `label` names what discovery asks,
/// for the header.
///
/// On quit the command channel closes; the worker notices at its next check
/// and exits. It is not joined: it may be mid-read, and making the user wait
/// out a request timeout to leave would be worse than letting the process
/// end. COV subscriptions it leaves behind lapse within their lifetime.
pub(crate) fn run(
    client: Client,
    plan: Discovery,
    label: String,
    interval: Duration,
) -> std::io::Result<()> {
    let (commands, worker_commands) = mpsc::channel();
    let (worker_events, events) = mpsc::channel();
    let worker = Worker::new(client, plan, worker_commands, worker_events);
    std::thread::Builder::new()
        .name("bacnet-worker".into())
        .spawn(move || worker.run())?;

    let (mut app, start) = App::new(label);
    app.interval = interval;
    let send = |cmds: Vec<worker::Command>| cmds.into_iter().for_each(|c| drop(commands.send(c)));
    send(vec![worker::Command::SetInterval(interval)]);
    send(start);

    ratatui::run(|terminal| -> std::io::Result<()> {
        while !app.quit {
            terminal.draw(|frame| ui::draw(frame, &mut app))?;
            if event::poll(TICK)?
                && let Input::Key(key) = event::read()?
            {
                send(app.on_key(key));
            }
            while let Ok(e) = events.try_recv() {
                send(app.on_event(e));
            }
        }
        Ok(())
    })
}
