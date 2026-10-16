// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The Packets tab (every datagram, dissected), the Log tab, and help.

use super::{pane, selected_style};
use crate::tui::app::App;
use crate::tui::keys::HELP;
use crate::tui::settings::edit::KEYS as SETTINGS_KEYS;
use bacnet_probe::{Direction, dissect};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Cell, Clear, Paragraph, Row, Table};

pub(super) fn packets(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Min(0)]).areas(area);
    let rows: Vec<Row<'_>> = app
        .packets
        .iter()
        .map(|p| {
            let arrow = match p.direction {
                Direction::Sent => Cell::from("→").fg(Color::Cyan),
                Direction::Received => Cell::from("←").fg(Color::Green),
            };
            Row::new([
                Cell::from(app.stamp(p.at)),
                arrow,
                Cell::from(p.peer.to_string()),
                Cell::from(p.bytes.len().to_string()),
                Cell::from(dissect::summary(&p.bytes)),
            ])
        })
        .collect();
    let follow = if app.follow {
        "following"
    } else {
        "f to follow"
    };
    let title = format!(" Packets ({}) {follow} ", app.packets.len());
    let widths = [
        Constraint::Length(10),
        Constraint::Length(1),
        Constraint::Length(21),
        Constraint::Length(4),
        Constraint::Min(0),
    ];
    let table = Table::new(rows, widths)
        .header(Row::new(["TIME", "", "PEER", "LEN", "SUMMARY"]).bold())
        .block(pane(title, true))
        .row_highlight_style(selected_style(true));
    frame.render_stateful_widget(table, left, &mut app.packet_table);

    let selected = app.packet_table.selected().and_then(|i| app.packets.get(i));
    let rows: Vec<Row<'_>> = selected
        .map(|p| dissect::frame(&p.bytes))
        .unwrap_or_default()
        .into_iter()
        .map(|field| {
            let mut hex: Vec<String> = field
                .bytes
                .iter()
                .take(8)
                .map(|b| format!("{b:02x}"))
                .collect();
            if field.bytes.len() > 8 {
                hex.push("…".into());
            }
            let meaning = format!("{}{}", "  ".repeat(field.depth), field.text);
            Row::new([
                Cell::from(hex.join(" ")).fg(Color::Yellow),
                Cell::from(meaning),
            ])
        })
        .collect();
    let table = Table::new(rows, [Constraint::Length(25), Constraint::Min(0)])
        .header(Row::new(["OCTETS", "FIELD"]).bold())
        .block(pane(" Dissection ".into(), false));
    frame.render_widget(table, right);
}

pub(super) fn log(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let height = usize::from(area.height.saturating_sub(2));
    let end = app.log.len().saturating_sub(app.log_scroll);
    let start = end.saturating_sub(height);
    let lines: Vec<Line<'_>> = app
        .log
        .range(start..end)
        .map(|(at, text)| {
            Line::from(vec![
                app.stamp(*at).dark_gray(),
                "  ".into(),
                text.clone().into(),
            ])
        })
        .collect();
    let title = if app.log_scroll > 0 {
        format!(" Log ({} lines, scrolled) ", app.log.len())
    } else {
        format!(" Log ({} lines) ", app.log.len())
    };
    frame.render_widget(Paragraph::new(lines).block(pane(title, true)), area);
}

pub(super) fn help(frame: &mut Frame<'_>) {
    let width = 72;
    let height = u16::try_from(HELP.len() + SETTINGS_KEYS.len() + 1)
        .unwrap_or(u16::MAX)
        .saturating_add(4);
    let area = frame.area();
    let popup = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width: width.min(area.width),
        height: height.min(area.height),
    };
    let key = |(k, what): &(&'static str, &'static str)| {
        Row::new([Cell::from(*k).fg(Color::Cyan).bold(), Cell::from(*what)])
    };
    let rows = HELP
        .iter()
        .map(key)
        .chain([Row::new(["", "On the Settings tab:"]).bold()])
        .chain(SETTINGS_KEYS.iter().map(key));
    let table = Table::new(rows, [Constraint::Length(16), Constraint::Min(0)])
        .block(pane(" Keys (esc to close) ".into(), true).style(Style::new().bg(Color::Black)));
    frame.render_widget(Clear, popup);
    frame.render_widget(table, popup);
}
