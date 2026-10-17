// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Changing a device through the app: from a key on the browse tab to the
//! command the worker receives, and back from the device's answer.

use super::loaded;
use crate::tui::app::{App, Focus};
use crate::tui::ui;
use crate::tui::worker::{Change, Command, Event};
use bacnet_probe::{ObjectId, Value};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::{Terminal, backend::TestBackend};

fn press(app: &mut App, codes: &[KeyCode]) -> Vec<Command> {
    codes
        .iter()
        .flat_map(|c| app.on_key(KeyEvent::from(*c)))
        .collect()
}

fn typed(s: &str) -> Vec<KeyCode> {
    s.chars().map(KeyCode::Char).collect()
}

#[test]
fn e_on_an_object_writes_its_present_value_after_confirmation() {
    let mut app = loaded();
    press(&mut app, &[KeyCode::Tab]);
    assert_eq!(app.focus, Focus::Objects);
    let mut codes = vec![KeyCode::Char('e')];
    codes.extend(typed("q500"));
    codes.push(KeyCode::Backspace);
    let none = press(&mut app, &codes);
    assert!(none.is_empty() && !app.quit, "q typed into the form");
    let sent = press(&mut app, &[KeyCode::Enter, KeyCode::Char('y')]);
    assert!(
        sent.is_empty(),
        "q50 is not a number; nothing sent: {sent:?}"
    );
    let mut fixed = vec![KeyCode::Esc, KeyCode::Char('e')];
    fixed.extend(typed("500"));
    fixed.extend([KeyCode::Enter, KeyCode::Char('y')]);
    let sent = press(&mut app, &fixed);
    let [Command::Change(Change::Write { object, values, .. })] = &sent[..] else {
        panic!("one write: {sent:?}");
    };
    assert_eq!(
        (*object, values.as_slice()),
        (ObjectId::new(0, 1), &[Value::Real(500.0)][..])
    );
}

#[test]
fn x_deletes_only_from_the_objects_pane() {
    let mut app = loaded();
    press(&mut app, &[KeyCode::Char('x')]);
    assert!(app.edit.is_none(), "the devices pane deletes nothing");
    press(&mut app, &[KeyCode::Tab, KeyCode::Char('x')]);
    let sent = press(&mut app, &[KeyCode::Char('y')]);
    assert!(matches!(
        &sent[..],
        [Command::Change(Change::Delete { .. })]
    ));
}

#[test]
fn a_refusal_is_shown_and_a_created_object_reloads_the_device() {
    let mut app = loaded();
    let refused = app.on_event(Event::Changed {
        instance: 599,
        what: "write analog-input:1 present-value".into(),
        structural: false,
        result: Err("property: write-access-denied".into()),
    });
    assert!(refused.is_empty());
    assert!(
        app.notice
            .as_ref()
            .is_some_and(|(_, t, ok)| !ok && t.contains("write-access-denied"))
    );
    let created = app.on_event(Event::Changed {
        instance: 599,
        what: "create analog-value".into(),
        structural: true,
        result: Ok(Some(ObjectId::new(2, 1))),
    });
    assert!(
        created
            .iter()
            .any(|c| matches!(c, Command::LoadDevice(d) if d.instance == 599))
    );
}

#[test]
fn the_form_and_its_confirmation_draw_over_the_browse_tab() {
    let mut app = loaded();
    let mut codes = vec![KeyCode::Tab, KeyCode::Char('e')];
    codes.extend(typed("500"));
    codes.push(KeyCode::Enter);
    press(&mut app, &codes);
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
        "Write present-value",
        "This changes the equipment",
        "y send",
    ] {
        assert!(screen.contains(want), "missing {want:?}");
    }
}
