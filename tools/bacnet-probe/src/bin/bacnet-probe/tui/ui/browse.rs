// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The Browse tab: devices, their objects, and the selected object's
//! properties.

use super::{FRESH, pane, selected_style};
use crate::tui::app::{App, Focus};
use bacnet_probe::{explain, names, property, props};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Stylize};
use ratatui::text::Span;
use ratatui::widgets::{Cell, Paragraph, Row, Table};

pub(super) fn browse(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let [left, right] =
        Layout::horizontal([Constraint::Length(36), Constraint::Min(0)]).areas(area);
    let [upper, lower] =
        Layout::vertical([Constraint::Percentage(58), Constraint::Min(0)]).areas(right);
    devices(frame, app, left);
    objects(frame, app, upper);
    properties(frame, app, lower);
}

fn devices(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let rows = app.devices.iter().map(|d| {
        let name = d
            .info
            .as_ref()
            .map(bacnet_probe::scan::ObjectReport::name)
            .unwrap_or_default();
        let route = if d.device.route.is_some() { "↳ " } else { "" };
        Row::new([
            Cell::from(d.device.instance.to_string()),
            Cell::from(format!("{route}{name}")),
        ])
    });
    let title = format!(" Devices ({}) ", app.devices.len());
    let table = Table::new(rows, [Constraint::Length(8), Constraint::Min(0)])
        .header(Row::new(["DEVICE", "NAME"]).bold())
        .block(pane(title, app.focus == Focus::Devices))
        .row_highlight_style(selected_style(app.focus == Focus::Devices));
    frame.render_stateful_widget(table, area, &mut app.device_table);
}

fn objects(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Objects;
    let Some(entry) = app.device() else {
        let hint = if app.discovering {
            "Waiting for I-Am replies…"
        } else {
            "No devices. Press r to discover."
        };
        frame.render_widget(
            Paragraph::new(hint).block(pane(" Objects ".into(), focused)),
            area,
        );
        return;
    };
    let rows: Vec<Row<'_>> = entry
        .objects
        .iter()
        .map(|row| {
            let o = &row.report;
            let value = match (o.present_value(), o.error(property::PRESENT_VALUE)) {
                (_, Some(e)) => Span::raw(format!("<{e}>")).red(),
                (Some(v), None) => {
                    let text = Span::raw(explain::present(o.id.kind, v, o.units(), &o.labels()));
                    if row.changed.is_some_and(|t| t.elapsed() < FRESH) {
                        text.fg(Color::Yellow).bold()
                    } else {
                        text
                    }
                }
                (None, None) => Span::raw(""),
            };
            Row::new([
                Cell::from(names::object(o.id)),
                Cell::from(o.name()),
                Cell::from(value),
                status(o),
            ])
        })
        .collect();
    let count = entry.count.map_or_else(|| "…".into(), |n| n.to_string());
    let title = format!(" Objects: device {} ({count}) ", entry.device.instance);
    let widths = [
        Constraint::Length(24),
        Constraint::Percentage(40),
        Constraint::Percentage(30),
        Constraint::Min(8),
    ];
    let table = Table::new(rows, widths)
        .header(Row::new(["OBJECT", "NAME", "VALUE", "STATUS"]).bold())
        .block(pane(title, focused))
        .row_highlight_style(selected_style(focused));
    frame.render_stateful_widget(table, area, &mut app.object_table);
}

fn status(o: &bacnet_probe::scan::ObjectReport) -> Cell<'static> {
    match o.get(111).and_then(|v| v.first()) {
        Some(flags) => {
            let text = explain::one(Some(111), Some(o.id.kind), flags);
            if text == "all clear" {
                Cell::from("ok").green()
            } else {
                Cell::from(text).red().bold()
            }
        }
        None => Cell::from(""),
    }
}

fn properties(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Properties;
    let target = app
        .detail_for
        .map(|(_, id)| names::object(id))
        .unwrap_or_default();
    let block = pane(format!(" Properties: {target} "), focused);
    let Some(detail) = &app.detail else {
        let hint = if app.detail_for.is_some() {
            "Reading…"
        } else {
            ""
        };
        frame.render_widget(Paragraph::new(hint).block(block), area);
        return;
    };
    let units = detail.units();
    let rows: Vec<Row<'_>> = detail
        .reads
        .iter()
        .map(|(prop, result)| {
            let value = match result {
                Ok(v) if *prop == property::PRESENT_VALUE => v
                    .first()
                    .map(|v| explain::present(detail.id.kind, v, units, &detail.labels()))
                    .unwrap_or_default(),
                Ok(v) => explain::many(*prop, detail.id.kind, v, units),
                Err(e) => format!("<{e}>"),
            };
            let value = value.trim_matches('"').to_owned();
            let cell = if result.is_err() {
                Cell::from(value).red()
            } else {
                Cell::from(value)
            };
            Row::new([Cell::from(props::name(*prop)), cell])
        })
        .collect();
    let table = Table::new(rows, [Constraint::Length(32), Constraint::Min(0)])
        .header(Row::new(["PROPERTY", "VALUE"]).bold())
        .block(block)
        .row_highlight_style(selected_style(focused));
    frame.render_stateful_widget(table, area, &mut app.property_table);
}
