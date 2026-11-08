//! The start screen: open by path, file dialog or drop; recent files; and databases other
//! running apps have open right now.

use std::path::{Path, PathBuf};

use rune::{Cursor, El, ScrollSpec, Tx, col, el, icon, row, text};
use rune_kit::{Button, ButtonVariant, ControlSize, IconButton, IconButtonVariant, Kit, Spinner, Switch, TextInput};

use crate::home;
use crate::theme::Ink;
use crate::view::{HEADING, RELOAD, TABLE};
use crate::{App, DB_EXTENSIONS};

/// Rows shown from the detected list before asking the user to filter.
const FOUND_LIMIT: usize = 40;

pub fn welcome(s: &App, k: &Kit, ink: &Ink) -> El {
    let intro = col()
        .w_full()
        .gap(12.0)
        .child(icon(&TABLE, 36.0).color(ink.bar))
        .child(text("Open a SQLite database").font(HEADING).color(k.c.text_base))
        .child(
            text(format!(
                "Type a path, choose a file, or drop a .{}, .{} or .{} file on this window. Databases open read-only.",
                DB_EXTENSIONS[0], DB_EXTENSIONS[1], DB_EXTENSIONS[2]
            ))
            .font(k.ty.body)
            .color(k.c.text_muted)
            .wrap(),
        );

    let field = TextInput::new(&s.path_edit)
        .label("Database path")
        .placeholder("~/Projects/app/data.db")
        .width(None)
        .large()
        .on_edit(|s: &mut App| &mut s.path_edit)
        .on_submit(|s: &mut App, _| s.open_typed())
        .build(k)
        .key("path")
        .grow(1.0)
        .min_w(0.0);
    let path_row = row()
        .w_full()
        .gap(8.0)
        .center_y()
        .child(field)
        .child(Button::new("Open").variant(ButtonVariant::Contrast).control_size(ControlSize::Large).on_press(|s: &mut App| s.open_typed()).build(k).key("open-path"));
    let buttons = row()
        .gap(8.0)
        .center_y()
        .child(
            Button::new("Choose a file…")
                .variant(ButtonVariant::Neutral)
                .shortcut("⌘O")
                .on_press_tx(|_: &mut App, tx: &mut Tx| App::choose_file(tx))
                .build(k)
                .key("choose"),
        )
        .child(Button::new("Open the demo").variant(ButtonVariant::Ghost).on_press(|s: &mut App| s.open(crate::demo_path())).build(k).key("demo"));

    let mut page = col().key("welcome").w_full().max_w(640.0).px(24.0).pt(24.0).pb(48.0).gap(16.0).child(intro).child(path_row).child(buttons);
    if s.schema_jobs.is_pending() {
        page = page.child(row().gap(8.0).center_y().child(Spinner::new().build(k)).child(text("Reading schema…").font(k.ty.small).color(k.c.text_muted)));
    }
    if let Some(e) = &s.error {
        page = page.child(text(e.clone()).font(k.ty.small).color(k.c.danger.icon).wrap().key("error"));
    }
    page = page.child(recents(s, k, ink)).child(detected(s, k, ink));

    col()
        .w_full()
        .h_full()
        .child(el().key("drag-band").w_full().h(40.0).window_drag())
        .child(
            col()
                .key("welcome-scroll")
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec { stick_bottom: false })
                .child(col().w_full().items_center().child(page)),
        )
}

fn section_header(k: &Kit, title: &str, trailing: El) -> El {
    row()
        .w_full()
        .pt(20.0)
        .pb(4.0)
        .gap(8.0)
        .center_y()
        .justify_between()
        .border_sides([0.0, 0.0, 1.0, 0.0], k.c.border_weak)
        .child(text(title.to_owned()).font(k.ty.ui_medium).color(k.c.text_base))
        .child(trailing)
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |f| f.to_string_lossy().into_owned())
}

fn folder(path: &Path) -> String {
    path.parent().map(home::tilde).unwrap_or_default()
}

/// One clickable file: name over its folder, with a note on the right.
fn file_row(k: &Kit, ink: &Ink, key: String, path: &Path, note: String, dim: bool, on_press: impl Fn(&mut App) + 'static) -> El {
    let name_color = if dim { k.c.text_faint } else { k.c.text_base };
    row()
        .key(key)
        .w_full()
        .h(46.0)
        .px(10.0)
        .gap(12.0)
        .center_y()
        .radius(k.radius.md)
        .hover(k.c.hover)
        .cursor(Cursor::Pointer)
        .role("button")
        .label(format!("Open {}", file_name(path)))
        .focusable(true)
        .on_press(on_press)
        .child(icon(&TABLE, 16.0).color(if dim { k.c.icon_faint } else { ink.bar }))
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(1.0)
                .child(text(file_name(path)).font(k.ty.ui_medium).color(name_color).truncate())
                .child(text(folder(path)).font(k.ty.small).color(k.c.text_faint).truncate()),
        )
        .child(text(note).font(k.ty.small).color(k.c.text_muted).truncate().max_w(220.0))
}

fn empty_line(k: &Kit, line: &str) -> El {
    text(line.to_owned()).font(k.ty.small).color(k.c.text_faint).wrap().pl(10.0).py(10.0)
}

fn recents(s: &App, k: &Kit, ink: &Ink) -> El {
    let clear = if s.recents.is_empty() {
        el()
    } else {
        Button::new("Clear")
            .variant(ButtonVariant::Ghost)
            .control_size(ControlSize::Small)
            .on_press(|s: &mut App| {
                s.recents.clear();
                home::save(&s.recents);
            })
            .build(k)
            .key("clear-recents")
    };
    let mut list = col().key("recents").w_full().gap(2.0).child(section_header(k, "Recent", clear));
    if s.recents.is_empty() {
        return list.child(empty_line(k, "Databases you open will be listed here."));
    }
    for (i, r) in s.recents.iter().enumerate() {
        let missing = !r.path.exists();
        let note = if missing { "Missing".to_owned() } else { home::ago(r.opened) };
        let path = r.path.clone();
        list = list.child(file_row(k, ink, format!("recent-{i}"), &r.path, note, missing, move |s: &mut App| s.open_recent(path.clone())));
    }
    list
}

fn matches(f: &home::Found, needle: &str) -> bool {
    needle.is_empty() || f.path.to_string_lossy().to_lowercase().contains(needle) || f.apps.iter().any(|a| a.to_lowercase().contains(needle))
}

fn detected(s: &App, k: &Kit, ink: &Ink) -> El {
    let refresh = IconButton::new(&RELOAD)
        .variant(IconButtonVariant::GhostMuted)
        .control_size(ControlSize::Small)
        .help("Look again")
        .on_press(|s: &mut App| s.detect())
        .build(k)
        .key("detect");
    let trailing = if s.found_jobs.is_pending() { row().gap(6.0).center_y().child(Spinner::new().build(k)).child(refresh) } else { refresh };
    let mut list = col().key("detected").w_full().gap(2.0).child(section_header(k, "Open in other apps", trailing));
    let found = match &s.found {
        None => return list.child(empty_line(k, "Looking for databases that running apps have open…")),
        Some(Err(e)) => return list.child(empty_line(k, e)),
        Some(Ok(found)) => found,
    };
    let system = found.iter().filter(|f| home::is_system(&f.path)).count();
    let filter = TextInput::new(&s.found_filter)
        .label("Filter databases")
        .placeholder("Filter by app or file")
        .width(None)
        .clear_button()
        .on_edit(|s: &mut App| &mut s.found_filter)
        .build(k)
        .key("found-filter")
        .grow(1.0)
        .min_w(0.0);
    let toggle = Switch::new(s.show_system).label(format!("Include system and app data ({system})")).on_toggle(|s: &mut App| s.show_system ^= true).build(k).key("show-system");
    list = list.child(row().w_full().gap(16.0).py(8.0).center_y().child(filter).child(toggle));

    let needle = s.found_filter.text.trim().to_lowercase();
    let shown: Vec<&home::Found> = found.iter().filter(|f| (s.show_system || !home::is_system(&f.path)) && matches(f, &needle)).collect();
    if shown.is_empty() {
        let line = if !needle.is_empty() {
            "Nothing matches that filter."
        } else if system > 0 {
            "No running app has a database open outside system and app-data folders. Turn on the switch above to see those."
        } else {
            "No running app has a SQLite database open right now."
        };
        return list.child(empty_line(k, line));
    }
    for (i, f) in shown.iter().take(FOUND_LIMIT).enumerate() {
        let path: PathBuf = f.path.clone();
        list = list.child(file_row(k, ink, format!("found-{i}"), &f.path, f.apps.join(", "), false, move |s: &mut App| s.open(path.clone())));
    }
    if shown.len() > FOUND_LIMIT {
        list = list.child(empty_line(k, &format!("{} more. Filter by app or file name to find one.", shown.len() - FOUND_LIMIT)));
    }
    list
}

