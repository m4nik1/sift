//! The window: a sidebar of tables and views, and a main pane with Data, Structure and SQL tabs.

use std::borrow::Cow;

use rune::{Align, Cx, El, Icon, ScrollSpec, TextStyle, Tx, col, el, row, text};
use rune_kit::shell::{PaneMotion, Sidebar, SidebarRow, SidebarSection, SidebarToggle, SplitView, Toolbar};
use rune_kit::{
    Button, ButtonVariant, Cell, syntax, ControlSize, IconButton, IconButtonVariant, Kit, SegmentedControl, Spinner, Table, TableColumn, TableState, TextArea,
};

use crate::db::{self, Class, Grid, ObjKind};
use crate::theme::{self, Ink};
use crate::{App, Tab};

pub static TABLE: Icon = Icon {
    name: "sift-table",
    view: 16.0,
    body: r#"<rect x="2.5" y="3" width="11" height="10" rx="1.5" stroke="currentColor" stroke-width="1.2" fill="none"/><path d="M2.5 6.5H13.5M6.5 6.5V13" stroke="currentColor" stroke-width="1.2"/>"#,
};
pub static VIEW: Icon = Icon {
    name: "sift-view",
    view: 16.0,
    body: r#"<rect x="2.5" y="3" width="11" height="10" rx="1.5" stroke="currentColor" stroke-width="1.2" stroke-dasharray="2 1.6" fill="none"/><path d="M5 8H11" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>"#,
};
pub static OPEN: Icon = Icon {
    name: "sift-open",
    view: 16.0,
    body: r#"<path d="M2.5 4.5C2.5 3.95 2.95 3.5 3.5 3.5H6.3L7.8 5H12.5C13.05 5 13.5 5.45 13.5 6V11.5C13.5 12.05 13.05 12.5 12.5 12.5H3.5C2.95 12.5 2.5 12.05 2.5 11.5Z" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round" fill="none"/>"#,
};
pub static RELOAD: Icon = Icon {
    name: "sift-reload",
    view: 16.0,
    body: r#"<path d="M12.5 8A4.5 4.5 0 1 1 11.2 4.8" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" fill="none"/><path d="M11.5 2.5V5H9" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
};
static PREV: Icon = Icon {
    name: "sift-prev",
    view: 16.0,
    body: r#"<path d="M9.5 4L5.5 8L9.5 12" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
};
static NEXT: Icon = Icon {
    name: "sift-next",
    view: 16.0,
    body: r#"<path d="M6.5 4L10.5 8L6.5 12" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
};

pub const HEADING: TextStyle = TextStyle::new(22.0, 600.0);

pub fn view(s: &App, cx: &mut Cx<App>) -> El {
    let dark = cx.system_dark();
    let k = theme::kit(dark);
    let ink = theme::ink(dark, &k);
    let body = if s.schema.is_some() { browser(s, cx, &k, &ink) } else { crate::welcome::welcome(s, &k, &ink) };
    let mut root = col()
        .key("root")
        .w_full()
        .h_full()
        .fill(k.c.page)
        .on_drop_files(|s: &mut App, files, _tx| {
            if let Some(f) = files.first() {
                s.drop_over = false;
                s.open(f.clone());
            }
        })
        .on_drop_hover(|s: &mut App, over, _tx| s.drop_over = over)
        .child(body);
    if s.drop_over {
        root = root.outline(2.0, -3.0, ink.bar).child(
            el().key("drop-wash").absolute(0.0, 0.0).w_full().h_full().fill(ink.drop).inert(),
        );
    }
    root
}

// ── Browser ─────────────────────────────────────────────────────────────────

fn browser(s: &App, cx: &mut Cx<App>, k: &Kit, ink: &Ink) -> El {
    let schema = s.schema.as_ref().unwrap();
    let obj = s.selected.and_then(|i| schema.objects.get(i));
    let title = obj.map_or("", |o| o.name.as_str());

    let tabs = SegmentedControl::new(&["Data", "Structure", "SQL"], Some(s.tab as usize))
        .label("View")
        .fit()
        .on_pick(|s: &mut App, i, _tx| {
            s.tab = [Tab::Data, Tab::Structure, Tab::Sql][i];
            if s.tab == Tab::Sql {
                s.sql.focused = true;
            }
        })
        .build(k)
        .key("tabs");
    let mut trailing = row().gap(4.0).center_y();
    if s.is_loading() {
        trailing = trailing.child(Spinner::new().build(k));
    }
    trailing = trailing
        .child(
            IconButton::new(&RELOAD)
                .variant(IconButtonVariant::GhostMuted)
                .help("Reload")
                .shortcut("⌘R")
                .on_press(|s: &mut App| s.reload())
                .build(k)
                .key("reload"),
        )
        .child(
            IconButton::new(&OPEN)
                .variant(IconButtonVariant::GhostMuted)
                .help("Open a database")
                .shortcut("⌘O")
                .on_press_tx(|_: &mut App, tx: &mut Tx| App::choose_file(tx))
                .build(k)
                .key("open"),
        );
    let toolbar = Toolbar::new().leading(text(title.to_owned()).font(k.ty.ui_medium).color(k.c.text_base).truncate()).center(tabs).trailing(trailing);

    let main = match s.tab {
        Tab::Data => data_pane(s, cx, k, ink),
        Tab::Structure => structure_pane(s, k, ink),
        Tab::Sql => sql_pane(s, cx, k, ink),
    };
    let main = match &s.error {
        Some(e) => col().w_full().h_full().child(error_strip(e, k)).child(col().w_full().grow(1.0).min_h(0.0).child(main)),
        None => main,
    };

    SplitView::new(main)
        .sidebar(&s.sidebar, |s: &mut App| &mut s.sidebar, sidebar(s, k, ink))
        .toolbar(toolbar)
        .titlebar_inset(cx.titlebar_inset())
        .frame_inset(8.0)
        .toolbar_in_panel()
        .sidebar_padding(14.0, 8.0)
        .sidebar_toggle(SidebarToggle::new().size(ControlSize::Large).past_lights(16.0).edge(18.0).gap(4.0).in_header())
        .pane_motion(PaneMotion::new().spring(rune::Spring::new(0.3, 0.0)).fade_tail(0.15))
        .reduced_motion(cx.is_reduced_motion())
        .build(k)
}

fn error_strip(e: &str, k: &Kit) -> El {
    row()
        .key("error")
        .w_full()
        .px(16.0)
        .py(8.0)
        .gap(10.0)
        .center_y()
        .fill(k.c.danger.bg)
        .child(text(e.to_owned()).font(k.ty.small).color(k.c.danger.fg).wrap().grow(1.0).min_w(0.0))
        .child(Button::new("Dismiss").variant(ButtonVariant::Ghost).control_size(ControlSize::Small).on_press(|s: &mut App| s.error = None).build(k).key("dismiss"))
}

fn short_count(n: u64) -> String {
    match n {
        n if n >= 1_000_000 => format!("{:.1}M", n as f64 / 1e6),
        n if n >= 10_000 => format!("{:.0}k", n as f64 / 1e3),
        n if n >= 1_000 => format!("{:.1}k", n as f64 / 1e3),
        n => n.to_string(),
    }
}

/// The row count, over a hairline whose length is this table's share of the largest one.
fn density(rows: Option<u64>, max: u64, k: &Kit, ink: &Ink) -> El {
    const W: f32 = 34.0;
    let label = rows.map_or("—".to_owned(), short_count);
    let frac = rows.map_or(0.0, |n| if max == 0 { 0.0 } else { (n as f64 / max as f64).sqrt() as f32 });
    col()
        .gap(3.0)
        .items_end()
        .child(text(label).font(k.ty.small).color(k.c.text_muted))
        .child(el().w(W).h(2.0).radius(1.0).fill(ink.bar_track).child(el().w((W * frac).max(if frac > 0.0 { 2.0 } else { 0.0 })).h(2.0).radius(1.0).fill(ink.bar)))
}

fn sidebar(s: &App, k: &Kit, ink: &Ink) -> El {
    let schema = s.schema.as_ref().unwrap();
    let file = schema.path.file_name().map_or_else(|| schema.path.display().to_string(), |f| f.to_string_lossy().into_owned());
    let max = schema.objects.iter().filter(|o| o.kind == ObjKind::Table).filter_map(|o| o.rows).max().unwrap_or(0);
    let header = col()
        .key("file")
        .w_full()
        .px(10.0)
        .pb(10.0)
        .gap(2.0)
        .child(text(file).font(k.ty.ui_medium).color(k.c.text_base).truncate())
        .child(text(format!("{} · SQLite {}", db::bytes(schema.file_size), schema.sqlite_version)).font(k.ty.small).color(k.c.text_faint).truncate());
    let row_for = |i: usize| {
        let o = &schema.objects[i];
        SidebarRow::new(o.name.clone())
            .icon(if o.kind == ObjKind::Table { &TABLE } else { &VIEW })
            .height(34.0)
            .selected(s.selected == Some(i))
            .trailing(density(o.rows, max, k, ink))
            .on_select(move |s: &mut App, _| s.select(i))
            .build(k)
            .key(format!("obj-{i}"))
    };
    let tables: Vec<usize> = (0..schema.objects.len()).filter(|&i| schema.objects[i].kind == ObjKind::Table).collect();
    let views: Vec<usize> = (0..schema.objects.len()).filter(|&i| schema.objects[i].kind == ObjKind::View).collect();
    let close = SidebarRow::new("Back to start").icon(&OPEN).height(32.0).on_select(|s: &mut App, _| s.close()).build(k).key("close-db");
    let mut sb = Sidebar::new().label("Database").header(header).footer(close);
    sb = sb.section(
        SidebarSection::new("Tables")
            .key("tables")
            .count(tables.len())
            .collapsed(s.tables_closed)
            .on_toggle(|s: &mut App, _| s.tables_closed ^= true)
            .rows(tables.iter().map(|&i| row_for(i))),
    );
    if !views.is_empty() {
        sb = sb.section(
            SidebarSection::new("Views")
                .key("views")
                .count(views.len())
                .collapsed(s.views_closed)
                .on_toggle(|s: &mut App, _| s.views_closed ^= true)
                .rows(views.iter().map(|&i| row_for(i))),
        );
    }
    sb.build(k)
}

// ── Grids ───────────────────────────────────────────────────────────────────

const ROWNUM: f32 = 56.0;

/// A table over `s.grid` (`results == false`) or `s.results`, scrolling sideways when its
/// columns are wider than the pane.
fn grid_table(s: &App, cx: &mut Cx<App>, k: &Kit, ink: &Ink, results: bool) -> El {
    type Get = fn(&App) -> &TableState;
    type GetMut = fn(&mut App) -> &mut TableState;
    type Len = fn(&App) -> usize;
    type RowId = for<'a> fn(&'a App, usize) -> Cow<'a, str>;
    fn grid_of(s: &App, results: bool) -> &Grid {
        if results { s.results.as_ref().unwrap() } else { s.grid.as_ref().unwrap() }
    }
    let (get, get_mut, len, row_id): (Get, GetMut, Len, RowId) = if results {
        (|s| &s.results_table, |s| &mut s.results_table, |s| s.results.as_ref().map_or(0, |g| g.rows.len()), |s, i| Cow::Borrowed(&s.results.as_ref().unwrap().ids[i]))
    } else {
        (|s| &s.table, |s| &mut s.table, |s| s.grid.as_ref().map_or(0, |g| g.rows.len()), |s, i| Cow::Borrowed(&s.grid.as_ref().unwrap().ids[i]))
    };
    let grid = grid_of(s, results);
    let frame_key = if results { "results-frame" } else { "grid-frame" };
    let avail = cx.layout_size(frame_key).map_or(0.0, |z| z[0]);
    let total: f32 = ROWNUM + grid.widths.iter().sum::<f32>();
    let fits = total <= avail;

    let mut table = Table::new(get, get_mut, len, row_id).name(if results { "Query results" } else { "Rows" }).zebra(true).column(
        TableColumn::new("#", "").width(ROWNUM).resizable(false).align(Align::End),
    );
    for (j, name) in grid.columns.iter().enumerate() {
        let w = grid.widths[j];
        let title = match grid.decls.get(j).filter(|d| !d.is_empty()) {
            Some(d) => format!("{name} · {}", d.to_lowercase()),
            None => name.clone(),
        };
        let mut c = TableColumn::new(j.to_string(), title).min(56f32.min(w));
        c = if fits { c.flex(w).min(w) } else { c.width(w).max(1200.0) };
        if grid.numeric[j] {
            c = c.align(Align::End);
        }
        if !results {
            c = c.sortable();
        }
        table = table.column(c);
    }
    let ink = ink.clone();
    let faint = k.c.text_faint;
    table = table.cell(move |s: &App, i, col| {
        let g = grid_of(s, results);
        if col == "#" {
            return Cell::text(db::thousands((g.offset + i + 1) as u64)).mono().color(faint);
        }
        let Some(v) = col.parse::<usize>().ok().and_then(|j| g.rows.get(i)?.get(j)) else { return Cell::text("") };
        if v.class == Class::Text && v.text.is_empty() {
            return Cell::text("\"\"").mono().color(faint);
        }
        let cell = Cell::text(v.text.clone()).color(ink.class(v.class));
        match v.class {
            Class::Integer | Class::Real | Class::Blob => cell.mono(),
            Class::Null => cell.mono(),
            Class::Text => cell,
        }
    });
    if !results {
        table = table.on_sort(|s: &mut App, col, dir, _| s.sort_by(col, dir));
    }
    let table = table.build(s, k).grow(1.0).min_h(0.0);
    let inner = if fits {
        table
    } else {
        row().key(if results { "results-hscroll" } else { "grid-hscroll" }).w_full().h_full().scroll_x().child(col().w(total).h_full().child(table))
    };
    col().key(frame_key).w_full().grow(1.0).min_h(0.0).child(inner)
}

fn status_bar(k: &Kit, left: El, right: El) -> El {
    row()
        .key("status")
        .w_full()
        .h(34.0)
        .px(14.0)
        .gap(12.0)
        .center_y()
        .justify_between()
        .border_sides([1.0, 0.0, 0.0, 0.0], k.c.border_weak)
        .child(left)
        .child(right)
}

fn centered_note(k: &Kit, title: &str, detail: &str) -> El {
    col()
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .center()
        .gap(6.0)
        .child(text(title.to_owned()).font(k.ty.ui_medium).color(k.c.text_base))
        .child(text(detail.to_owned()).font(k.ty.small).color(k.c.text_muted).wrap().max_w(360.0).align(Align::Center))
}

fn data_pane(s: &App, cx: &mut Cx<App>, k: &Kit, ink: &Ink) -> El {
    let Some(grid) = &s.grid else {
        return col().w_full().h_full().center().child(Spinner::new().build(k));
    };
    let body = if grid.rows.is_empty() && s.offset == 0 {
        centered_note(k, "No rows", "This table is empty. Its columns are listed under Structure.")
    } else {
        grid_table(s, cx, k, ink, false)
    };
    let total = s.total_rows();
    let first = s.offset + if grid.rows.is_empty() { 0 } else { 1 };
    let last = s.offset + grid.rows.len();
    let range = match total {
        Some(t) => format!("Rows {}–{} of {}", db::thousands(first as u64), db::thousands(last as u64), db::thousands(t)),
        None => format!("Rows {}–{}", db::thousands(first as u64), db::thousands(last as u64)),
    };
    let mut left = row().gap(14.0).center_y().child(text(range).font(k.ty.small).color(k.c.text_muted));
    if let Some((name, desc)) = &s.sort {
        left = left.child(text(format!("Sorted by {name}, {}", if *desc { "descending" } else { "ascending" })).font(k.ty.small).color(k.c.text_faint));
    }
    left = left.child(text(format!("{:.1} ms", grid.millis)).font(k.ty.small).color(k.c.text_faint));
    let has_prev = s.offset > 0;
    let has_next = total.map_or(grid.rows.len() == db::PAGE, |t| (last as u64) < t);
    let pager = row()
        .gap(2.0)
        .center_y()
        .child(
            IconButton::new(&PREV)
                .variant(IconButtonVariant::GhostMuted)
                .control_size(ControlSize::Small)
                .help("Previous page")
                .shortcut("⌘[")
                .disabled(!has_prev)
                .on_press(|s: &mut App| s.page_by(-1))
                .build(k)
                .key("prev"),
        )
        .child(
            IconButton::new(&NEXT)
                .variant(IconButtonVariant::GhostMuted)
                .control_size(ControlSize::Small)
                .help("Next page")
                .shortcut("⌘]")
                .disabled(!has_next)
                .on_press(|s: &mut App| s.page_by(1))
                .build(k)
                .key("next"),
        );
    col().w_full().h_full().child(body).child(status_bar(k, left, pager))
}

/// The ink a declared column type gets, by SQLite's affinity rules.
fn affinity_ink(decl: &str, ink: &Ink) -> rune::Color {
    let d = decl.to_ascii_uppercase();
    if d.contains("INT") {
        ink.integer
    } else if d.contains("CHAR") || d.contains("CLOB") || d.contains("TEXT") {
        ink.text
    } else if d.is_empty() || d.contains("BLOB") {
        ink.blob
    } else if d.contains("REAL") || d.contains("FLOA") || d.contains("DOUB") {
        ink.real
    } else {
        ink.integer
    }
}

fn section(k: &Kit, title: &str, body: El) -> El {
    col().w_full().gap(10.0).child(text(title.to_owned()).font(k.ty.ui_medium).color(k.c.text_base)).child(body)
}

fn code_block(k: &Kit, src: &str) -> El {
    col()
        .w_full()
        .padding(14.0)
        .radius(k.radius.md)
        .fill(k.c.layer_01)
        .border(1.0, k.c.border_weak)
        .child(text(src.to_owned()).font(k.ty.mono).color(k.c.text_base).wrap().selectable(true).selection_color(k.selection()))
}

fn structure_pane(s: &App, k: &Kit, ink: &Ink) -> El {
    let schema = s.schema.as_ref().unwrap();
    let Some(obj) = s.selected.and_then(|i| schema.objects.get(i)) else { return centered_note(k, "Nothing selected", "Choose a table in the sidebar.") };
    let mut cols = col().w_full().radius(k.radius.md).border(1.0, k.c.border_weak).clip_rounded(k.radius.md);
    for (i, c) in obj.columns.iter().enumerate() {
        let mut flags = Vec::new();
        if c.pk {
            flags.push("primary key");
        }
        if c.not_null {
            flags.push("not null");
        }
        let mut r = row()
            .key(format!("col-{i}"))
            .w_full()
            .h(34.0)
            .px(14.0)
            .gap(12.0)
            .center_y()
            .child(text(c.name.clone()).font(k.ty.mono).color(k.c.text_base).w(220.0).truncate())
            .child(text(if c.decl.is_empty() { "any".to_owned() } else { c.decl.to_lowercase() }).font(k.ty.mono).color(affinity_ink(&c.decl, ink)).w(160.0).truncate())
            .child(text(flags.join(", ")).font(k.ty.small).color(k.c.text_muted).grow(1.0).min_w(0.0).truncate());
        if i > 0 {
            r = r.border_sides([1.0, 0.0, 0.0, 0.0], k.c.border_weak);
        }
        cols = cols.child(r);
    }
    let kind = if obj.kind == ObjKind::Table { "table" } else { "view" };
    let summary = format!(
        "A {kind} with {} column{} and {} row{}.",
        obj.columns.len(),
        if obj.columns.len() == 1 { "" } else { "s" },
        obj.rows.map_or("an unknown number of".into(), db::thousands),
        if obj.rows == Some(1) { "" } else { "s" }
    );
    let mut page = col()
        .key("structure")
        .w_full()
        .max_w(860.0)
        .padding(24.0)
        .gap(28.0)
        .child(text(summary).font(k.ty.body).color(k.c.text_muted))
        .child(section(k, "Columns", cols));
    if !obj.indexes.is_empty() {
        let mut list = col().w_full().gap(8.0);
        for (name, sql) in &obj.indexes {
            list = list.child(if sql.is_empty() {
                text(format!("{name} (created by a constraint)")).font(k.ty.mono).color(k.c.text_muted)
            } else {
                code_block(k, sql)
            });
        }
        page = page.child(section(k, "Indexes", list));
    }
    page = page.child(section(k, "Definition", code_block(k, &obj.sql)));
    col().key("structure-scroll").w_full().h_full().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(page)
}

fn sql_pane(s: &App, cx: &mut Cx<App>, k: &Kit, ink: &Ink) -> El {
    let editor = TextArea::new(&s.sql)
        .label("SQL")
        .placeholder("SELECT * FROM …")
        .width(None)
        .auto_grow(4, 12)
        .decorations(syntax::SyntaxTheme::from_kit(k).spans(&syntax::highlight(Some("sql"), &s.sql.text), 0))
        .on_edit(|s: &mut App| &mut s.sql)
        .on_submit(|s: &mut App, _| s.run_query())
        .build(k)
        .key("sql");
    let run = Button::new("Run")
        .variant(ButtonVariant::Contrast)
        .shortcut("⌘↩")
        .on_press(|s: &mut App| s.run_query())
        .build(k)
        .key("run");
    let bar = row()
        .w_full()
        .gap(12.0)
        .center_y()
        .child(text("Read-only: queries that change data are refused.").font(k.ty.small).color(k.c.text_faint).grow(1.0).min_w(0.0))
        .child(run);
    let top = col().key("sql-top").w_full().padding(16.0).gap(10.0).border_sides([0.0, 0.0, 1.0, 0.0], k.c.border_weak).child(editor).child(bar);
    let (body, status) = if let Some(e) = &s.query_error {
        (
            col().w_full().grow(1.0).min_h(0.0).padding(16.0).child(text(e.clone()).font(k.ty.mono).color(k.c.danger.icon).wrap().selectable(true)),
            text("The query failed").font(k.ty.small).color(k.c.text_muted),
        )
    } else if let Some(g) = &s.results {
        let body = if g.columns.is_empty() { centered_note(k, "Done", "The statement returned no columns.") } else { grid_table(s, cx, k, ink, true) };
        let more = if g.truncated { format!(" (first {})", db::thousands(db::QUERY_LIMIT as u64)) } else { String::new() };
        (body, text(format!("{} row{}{more} · {:.1} ms", db::thousands(g.rows.len() as u64), if g.rows.len() == 1 { "" } else { "s" }, g.millis)).font(k.ty.small).color(k.c.text_muted))
    } else {
        (centered_note(k, "Run a query to see results", "Press ⌘↩ in the editor, or click Run."), text("").font(k.ty.small))
    };
    let spinner = if s.query_jobs.is_pending() { Spinner::new().build(k) } else { el() };
    col().w_full().h_full().child(top).child(body).child(status_bar(k, status, spinner))
}
