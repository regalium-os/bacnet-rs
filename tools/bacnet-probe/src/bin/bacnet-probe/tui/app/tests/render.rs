// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! One frame rendered to an in-memory terminal, to prove the panes draw.

use super::loaded;
use crate::tui::app::Tab;
use crate::tui::ui;
use ratatui::{Terminal, backend::TestBackend};

#[test]
fn a_frame_draws_every_browse_pane_and_the_packets_tab() {
    let mut app = loaded();
    let mut terminal = Terminal::new(TestBackend::new(140, 30)).unwrap();
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let screen: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    for want in [
        "Devices (1)",
        "Lift gateway",
        "Car 1 load",
        "420.00",
        "analog-input:1",
        "Properties",
    ] {
        assert!(screen.contains(want), "missing {want:?}");
    }
    app.tab = Tab::Packets;
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let screen: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    assert!(screen.contains("Packets (0)") && screen.contains("Dissection"));
}
