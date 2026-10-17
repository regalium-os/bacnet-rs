// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Drawing: the frame (tabs, status, key hints). The Browse tab is in
//! `browse`; the Packets and Log tabs and the help overlay in `more`; the
//! Settings tab in `settings`.

mod browse;
mod edit;
mod more;
mod settings;

use super::app::{App, Load, Tab};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Tabs};
use std::time::Duration;

/// How long a changed value stays highlighted.
pub(crate) const FRESH: Duration = Duration::from_secs(3);

/// How long the outcome of a change stays in the footer.
const NOTICE: Duration = Duration::from_secs(6);

pub(crate) fn draw(frame: &mut Frame<'_>, app: &mut App) {
    let [top, body, bottom] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    header(frame, app, top);
    match app.tab {
        Tab::Browse => browse::browse(frame, app, body),
        Tab::Packets => more::packets(frame, app, body),
        Tab::Log => more::log(frame, app, body),
        Tab::Settings => settings::settings(frame, app, body),
    }
    footer(frame, app, bottom);
    if app.help {
        more::help(frame);
    }
    if let Some(e) = &app.edit {
        edit::draw(frame, e);
    }
}

fn header(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let tabs = Tabs::new(["1 Browse", "2 Packets", "3 Log", "4 Settings"])
        .select(match app.tab {
            Tab::Browse => 0,
            Tab::Packets => 1,
            Tab::Log => 2,
            Tab::Settings => 3,
        })
        .highlight_style(Style::new().fg(Color::Black).bg(Color::Cyan).bold())
        .divider(" ");
    let [left, right] =
        Layout::horizontal([Constraint::Length(44), Constraint::Min(0)]).areas(area);
    frame.render_widget(tabs, left);
    let watch = match (app.watching, app.following) {
        (false, _) => "live off".into(),
        (true, Some((cov, polled))) => {
            format!(
                "live: {cov} by COV, {polled} polled every {:?}",
                app.interval
            )
        }
        (true, None) => format!("live: polling every {:?}", app.interval),
    };
    let status = Line::from(vec![
        Span::raw("bacnet-probe ").bold(),
        Span::raw(format!("-> {}  ", app.target)).dark_gray(),
        Span::raw(watch).fg(if app.watching {
            Color::Green
        } else {
            Color::DarkGray
        }),
    ]);
    frame.render_widget(Paragraph::new(status).right_aligned(), right);
}

fn footer(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let mut spans = Vec::new();
    if app.discovering {
        spans.push(
            Span::raw(" discovering… ")
                .fg(Color::Black)
                .bg(Color::Yellow),
        );
    } else if let Some(d) = app.device().filter(|d| d.load == Load::Loading) {
        let progress = d
            .count
            .map_or_else(String::new, |n| format!(" {}/{n}", d.objects.len()));
        spans.push(
            Span::raw(format!(" loading device {}{progress} ", d.device.instance))
                .fg(Color::Black)
                .bg(Color::Yellow),
        );
    }
    if let Some((_, text, ok)) = app.notice.as_ref().filter(|(at, ..)| at.elapsed() < NOTICE) {
        let colour = if *ok { Color::Green } else { Color::Red };
        spans.push(Span::raw(format!(" {text} ")).fg(Color::Black).bg(colour));
    }
    for (key, what) in [
        ("?", "help"),
        ("tab", "focus"),
        ("enter", "reload"),
        ("r", "discover"),
        ("w", "live"),
        ("e", "write"),
        ("s", "settings"),
        ("q", "quit"),
    ] {
        spans.push(Span::raw(format!(" {key} ")).bold().fg(Color::Cyan));
        spans.push(Span::raw(what).dark_gray());
    }
    frame.render_widget(Line::from(spans), area);
}

pub(crate) fn pane(title: String, focused: bool) -> Block<'static> {
    let style = if focused {
        Style::new().fg(Color::Cyan)
    } else {
        Style::new().fg(Color::DarkGray)
    };
    Block::bordered()
        .border_style(style)
        .title(Span::styled(title, style.add_modifier(Modifier::BOLD)))
}

pub(crate) fn selected_style(focused: bool) -> Style {
    if focused {
        Style::new().bg(Color::Cyan).fg(Color::Black)
    } else {
        Style::new().bg(Color::DarkGray)
    }
}
