// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Key bindings. Every binding is listed in `HELP`, which the help overlay
//! renders, so the two cannot drift apart.

use super::app::{App, Focus, Tab};
use super::worker::Command;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::widgets::TableState;
use std::time::Duration;

/// Focus order for Tab and the arrow keys.
const ORDER: [Focus; 3] = [Focus::Devices, Focus::Objects, Focus::Properties];

/// `at` moved by `delta`, clamped to `0..len`.
fn moved(at: usize, delta: isize, len: usize) -> usize {
    let last = len.saturating_sub(1);
    if delta < 0 {
        at.saturating_sub(delta.unsigned_abs())
    } else {
        at.saturating_add(delta.unsigned_abs()).min(last)
    }
}

pub(crate) const HELP: &[(&str, &str)] = &[
    ("1 2 3", "Browse / Packets / Log tab"),
    ("Tab, ← →", "move focus: devices, objects, properties"),
    ("↑ ↓  j k", "move selection"),
    ("PgUp PgDn", "move ten rows"),
    ("g G  Home End", "first / last row"),
    ("Enter", "reload the selected device or object"),
    ("r", "re-run discovery (Who-Is)"),
    (
        "w",
        "start / stop following live values (COV, else polling)",
    ),
    ("+ -", "poll faster / slower (points without COV)"),
    ("f", "packets: follow the newest"),
    ("c", "packets / log: clear"),
    ("?", "this help"),
    ("q  Ctrl-C", "quit"),
];

impl App {
    pub(crate) fn on_key(&mut self, key: KeyEvent) -> Vec<Command> {
        if key.kind != KeyEventKind::Press {
            return Vec::new();
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.quit = true;
            return Vec::new();
        }
        if self.help {
            self.help = !matches!(
                key.code,
                KeyCode::Esc | KeyCode::Char('?' | 'q') | KeyCode::Enter
            );
            return Vec::new();
        }
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => self.help = true,
            KeyCode::Char('1') => self.tab = Tab::Browse,
            KeyCode::Char('2') => self.tab = Tab::Packets,
            KeyCode::Char('3') => self.tab = Tab::Log,
            KeyCode::Char('r') => return vec![Command::Discover],
            KeyCode::Char('w') => {
                self.watching = !self.watching;
                self.log(format!(
                    "polling {}",
                    if self.watching { "on" } else { "off" }
                ));
                return self.watch_command().into_iter().collect();
            }
            KeyCode::Char('+' | '=') => return self.set_interval(self.interval / 2),
            KeyCode::Char('-' | '_') => return self.set_interval(self.interval * 2),
            KeyCode::Char('f') => self.follow = !self.follow,
            KeyCode::Char('c') => match self.tab {
                Tab::Packets => {
                    self.packets.clear();
                    self.packet_table = TableState::default();
                }
                Tab::Log => self.log.clear(),
                Tab::Browse => {}
            },
            KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => return self.focus_by(1),
            KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => return self.focus_by(-1),
            KeyCode::Enter => return self.enter(),
            KeyCode::Down | KeyCode::Char('j') => return self.step(1),
            KeyCode::Up | KeyCode::Char('k') => return self.step(-1),
            KeyCode::PageDown => return self.step(10),
            KeyCode::PageUp => return self.step(-10),
            KeyCode::Home | KeyCode::Char('g') => return self.step(isize::MIN / 2),
            KeyCode::End | KeyCode::Char('G') => return self.step(isize::MAX / 2),
            _ => {}
        }
        Vec::new()
    }

    fn set_interval(&mut self, interval: Duration) -> Vec<Command> {
        self.interval = interval.clamp(Duration::from_millis(250), Duration::from_secs(60));
        self.log(format!("polling every {:?}", self.interval));
        vec![Command::SetInterval(self.interval)]
    }

    fn focus_by(&mut self, delta: isize) -> Vec<Command> {
        if self.tab != Tab::Browse {
            return Vec::new();
        }
        let at = ORDER.iter().position(|f| *f == self.focus).unwrap_or(0);
        let next = if delta < 0 {
            (at + ORDER.len() - 1) % ORDER.len()
        } else {
            (at + 1) % ORDER.len()
        };
        self.focus = ORDER.get(next).copied().unwrap_or(Focus::Devices);
        if self.focus == Focus::Objects
            && self.object_table.selected().is_none()
            && self.device().is_some_and(|d| !d.objects.is_empty())
        {
            self.object_table.select(Some(0));
        }
        self.detail_command().into_iter().collect()
    }

    fn enter(&mut self) -> Vec<Command> {
        match (self.tab, self.focus) {
            (Tab::Browse, Focus::Devices) => {
                let Some(entry) = self
                    .device_table
                    .selected()
                    .and_then(|i| self.devices.get_mut(i))
                else {
                    return Vec::new();
                };
                entry.load = super::app::Load::NotLoaded;
                entry.objects.clear();
                self.device_selected()
            }
            (Tab::Browse, _) => self.reload_detail().into_iter().collect(),
            _ => Vec::new(),
        }
    }

    /// Moves the selection in whatever list has focus, clamped to its length.
    fn step(&mut self, delta: isize) -> Vec<Command> {
        let move_in = |state: &mut TableState, len: usize| {
            if len == 0 {
                return false;
            }
            let to = moved(state.selected().unwrap_or(0), delta, len);
            let changed = state.selected() != Some(to);
            state.select(Some(to));
            changed
        };
        match self.tab {
            Tab::Packets => {
                self.follow = false;
                move_in(&mut self.packet_table, self.packets.len());
            }
            Tab::Log => {
                // Scrolled is lines back from the newest; up goes further back.
                self.log_scroll = moved(self.log_scroll, -delta, self.log.len() + 1);
            }
            Tab::Browse => match self.focus {
                Focus::Devices => {
                    if move_in(&mut self.device_table, self.devices.len()) {
                        return self.device_selected();
                    }
                }
                Focus::Objects => {
                    let len = self.device().map_or(0, |d| d.objects.len());
                    if move_in(&mut self.object_table, len) {
                        return self.detail_command().into_iter().collect();
                    }
                }
                Focus::Properties => {
                    let len = self.detail.as_ref().map_or(0, |d| d.reads.len());
                    move_in(&mut self.property_table, len);
                }
            },
        }
        Vec::new()
    }
}
