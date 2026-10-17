// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `bacnet-probe tui`: browse devices, objects and properties live, and
//! watch every packet go by, dissected.
//!
//! Two threads. The worker owns the socket and does all network I/O; this
//! thread owns the terminal and never blocks on the wire. They talk over two
//! channels: commands down, events up.

mod app;
mod edit;
mod keys;
mod settings;
mod ui;
mod worker;

use crate::args::Net;
use crate::connect::{Connector, Opened};
use app::App;
use ratatui::crossterm::event::{self, Event as Input};
use std::sync::mpsc;
use std::time::Duration;
use worker::Worker;

/// How often the screen redraws when nothing happens (highlights fade).
const TICK: Duration = Duration::from_millis(100);

/// Runs the TUI until the user quits, starting from `opened`, which `net`
/// describes. The Settings tab reopens through `connect`.
///
/// On quit the command channel closes; the worker notices at its next check
/// and exits. It is not joined: it may be mid-read, and making the user wait
/// out a request timeout to leave would be worse than letting the process
/// end. COV subscriptions it leaves behind lapse within their lifetime.
pub(crate) fn run(
    opened: Opened,
    net: Net,
    interval: Duration,
    connect: Connector,
) -> std::io::Result<()> {
    let (commands, worker_commands) = mpsc::channel();
    let (worker_events, events) = mpsc::channel();
    let Opened {
        client,
        plan,
        notes,
    } = opened;
    let worker = Worker::new(client, plan, worker_commands, worker_events)
        .with_connector(net.clone(), connect);
    std::thread::Builder::new()
        .name("bacnet-worker".into())
        .spawn(move || worker.run())?;

    let (app, start) = App::new(String::new());
    let mut app = app.configured(net, interval, notes);
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
