//! Sift: a clean, read-only SQLite viewer built with Rune.
//!
//! `sift [path/to/file.db]` opens a database (or drop one on the window, or press ⌘O).
//! Headless: `sift --snap out.png --scene demo` renders the bundled demo scene.

mod db;
mod home;
mod theme;
mod view;
mod welcome;

use std::path::PathBuf;

use rune::{AppSpec, Chrome, FileDialogOptions, KeyEvent, Latest, TextEdit, Tx, Waker};
use rune_kit::shell::{self, PaneState};
use rune_kit::{SortDir, TableState};

use db::{Grid, PageRequest, Schema};

pub const DB_EXTENSIONS: &[&str] = &["db", "sqlite", "sqlite3", "db3", "s3db", "sl3"];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Data,
    Structure,
    Sql,
}

pub struct App {
    pub path: Option<PathBuf>,
    pub schema: Option<Schema>,
    pub selected: Option<usize>,
    pub tab: Tab,

    pub grid: Option<Grid>,
    pub table: TableState,
    pub sort: Option<(String, bool)>,
    pub offset: usize,

    pub sql: TextEdit,
    pub results: Option<Grid>,
    pub results_table: TableState,
    pub query_error: Option<String>,

    pub error: Option<String>,
    pub sidebar: PaneState,
    pub drop_over: bool,
    pub tables_closed: bool,
    pub views_closed: bool,

    /// The start screen: recently opened files, a typed path, and databases other apps have open.
    pub recents: Vec<home::Recent>,
    pub path_edit: TextEdit,
    pub found: Option<Result<Vec<home::Found>, String>>,
    pub found_filter: TextEdit,
    pub show_system: bool,

    pub schema_jobs: Latest<Result<Schema, String>>,
    pub found_jobs: Latest<Result<Vec<home::Found>, String>>,
    pub page_jobs: Latest<Result<Grid, String>>,
    pub query_jobs: Latest<Result<Grid, String>>,
}

fn init() -> App {
    let path = rune::extra_args().into_iter().find(|a| !a.starts_with('-')).map(PathBuf::from);
    let mut path_edit = TextEdit::new("");
    path_edit.focused = path.is_none();
    App {
        recents: home::load_recents(),
        path_edit,
        found: None,
        found_filter: TextEdit::new(""),
        show_system: false,
        found_jobs: Latest::default(),
        path,
        schema: None,
        selected: None,
        tab: Tab::Data,
        grid: None,
        table: TableState::new().key("data"),
        sort: None,
        offset: 0,
        sql: TextEdit::new(""),
        results: None,
        results_table: TableState::new().key("results"),
        query_error: None,
        error: None,
        sidebar: PaneState::new(260.0).range(180.0, 460.0),
        drop_over: false,
        tables_closed: false,
        views_closed: false,
        schema_jobs: Latest::default(),
        page_jobs: Latest::default(),
        query_jobs: Latest::default(),
    }
}

fn setup(s: &mut App, waker: Waker) {
    s.schema_jobs.set_waker(waker.clone());
    s.found_jobs.set_waker(waker.clone());
    s.page_jobs.set_waker(waker.clone());
    s.query_jobs.set_waker(waker);
    match s.path.clone() {
        Some(path) => s.open(path),
        None => s.detect(),
    }
}

impl App {
    /// Starts loading the database at `path`; its tables appear when the schema lands.
    pub fn open(&mut self, path: PathBuf) {
        self.path = Some(path.clone());
        self.error = None;
        self.schema_jobs.request(path, db::load_schema);
    }

    /// Opens what the user typed in the path field, or says why it can't.
    pub fn open_typed(&mut self) {
        let Some(path) = home::typed_path(&self.path_edit.text) else {
            self.error = Some("Type or paste the path to a database file.".into());
            return;
        };
        if path.is_dir() {
            self.error = Some(format!("{} is a folder. Type the path to a database file inside it.", home::tilde(&path)));
        } else if !path.exists() {
            self.error = Some(format!("There's no file at {}.", home::tilde(&path)));
        } else {
            self.open(path);
        }
    }

    pub fn open_recent(&mut self, path: PathBuf) {
        if path.exists() {
            self.open(path);
        } else {
            self.error = Some(format!("{} has moved or been deleted, so it was removed from Recent.", home::tilde(&path)));
            home::forget(&mut self.recents, &path);
        }
    }

    /// Looks for databases other apps have open, off the main thread.
    pub fn detect(&mut self) {
        self.found_jobs.request((), |()| home::detect());
    }

    /// Closes the database and goes back to the start screen.
    pub fn close(&mut self) {
        self.schema_jobs.cancel();
        self.page_jobs.cancel();
        self.query_jobs.cancel();
        self.schema = None;
        self.path = None;
        self.grid = None;
        self.selected = None;
        self.results = None;
        self.query_error = None;
        self.error = None;
        self.sql = TextEdit::new("");
        self.path_edit = TextEdit::new("");
        self.path_edit.focused = true;
        self.detect();
    }

    pub fn reload(&mut self) {
        if let Some(path) = self.path.clone() {
            self.open(path);
        }
    }

    /// Lands a loaded schema: keeps the selected table when it still exists.
    fn land_schema(&mut self, schema: Schema) {
        let keep = self.selected_name().map(str::to_owned);
        let same_file = self.schema.as_ref().is_some_and(|old| old.path == schema.path);
        let index = keep.filter(|_| same_file).and_then(|n| schema.objects.iter().position(|o| o.name == n));
        if !same_file {
            self.sql = TextEdit::new("");
            self.results = None;
            self.query_error = None;
        }
        if schema.path != demo_path() {
            home::remember(&mut self.recents, &schema.path);
        }
        self.schema = Some(schema);
        self.selected = None;
        self.select(index.unwrap_or(0));
    }

    pub fn selected_name(&self) -> Option<&str> {
        let s = self.schema.as_ref()?;
        Some(s.objects.get(self.selected?)?.name.as_str())
    }

    pub fn select(&mut self, index: usize) {
        let Some(schema) = &self.schema else { return };
        if index >= schema.objects.len() {
            self.selected = None;
            self.grid = None;
            return;
        }
        if self.selected != Some(index) {
            self.sort = None;
            self.offset = 0;
            self.table = TableState::new().key("data");
        }
        self.selected = Some(index);
        let obj = &schema.objects[index];
        if self.sql.text.trim().is_empty() {
            self.sql = TextEdit::new(format!("SELECT *\nFROM {}\nLIMIT 100;", db::quote(&obj.name)));
        }
        self.request_page();
    }

    fn request_page(&mut self) {
        let (Some(schema), Some(i)) = (&self.schema, self.selected) else { return };
        let obj = &schema.objects[i];
        let req = PageRequest {
            path: schema.path.clone(),
            table: obj.name.clone(),
            decls: obj.columns.iter().map(|c| c.decl.clone()).collect(),
            sort: self.sort.clone(),
            offset: self.offset,
        };
        self.page_jobs.request(req, db::load_page);
    }

    pub fn sort_by(&mut self, column: &str, dir: SortDir) {
        let Some(grid) = &self.grid else { return };
        let Ok(j) = column.parse::<usize>() else { return };
        let Some(name) = grid.columns.get(j) else { return };
        self.sort = Some((name.clone(), dir == SortDir::Descending));
        self.offset = 0;
        self.request_page();
    }

    pub fn total_rows(&self) -> Option<u64> {
        let s = self.schema.as_ref()?;
        s.objects.get(self.selected?)?.rows
    }

    pub fn page_by(&mut self, pages: isize) {
        let total = self.total_rows().unwrap_or(u64::MAX) as usize;
        let next = self.offset as isize + pages * db::PAGE as isize;
        if next < 0 || next as usize >= total {
            return;
        }
        self.offset = next as usize;
        self.request_page();
    }

    pub fn run_query(&mut self) {
        let Some(schema) = &self.schema else { return };
        self.query_jobs.request((schema.path.clone(), self.sql.text.clone()), |(p, q)| db::run_query(p, q));
    }

    pub fn choose_file(tx: &mut Tx) {
        let options = FileDialogOptions { title: "Open a SQLite database".into(), ..Default::default() }.filter("SQLite database", DB_EXTENSIONS);
        tx.open_file(options, |s: &mut App, result, _tx| match result {
            Ok(Some(path)) => s.open(path),
            Ok(None) => {}
            Err(e) => s.error = Some(format!("Couldn't show the open dialog: {e}")),
        });
    }

    pub fn is_loading(&self) -> bool {
        self.schema_jobs.is_pending() || self.page_jobs.is_pending()
    }
}

fn poll(s: &mut App, _: f64, _: &mut Tx) -> bool {
    let mut changed = false;
    if let Some(r) = s.schema_jobs.take() {
        match r {
            Ok(schema) => s.land_schema(schema),
            Err(e) => s.error = Some(e),
        }
        changed = true;
    }
    if let Some(r) = s.found_jobs.take() {
        s.found = Some(r);
        changed = true;
    }
    if let Some(r) = s.page_jobs.take() {
        match r {
            Ok(grid) => {
                // Mirror our sort in the header so a new page keeps showing it.
                if let Some((name, desc)) = &s.sort {
                    if let Some(j) = grid.columns.iter().position(|c| c == name) {
                        s.table.set_sort(j.to_string(), if *desc { SortDir::Descending } else { SortDir::Ascending });
                    }
                }
                s.grid = Some(grid);
            }
            Err(e) => {
                s.grid = None;
                s.error = Some(e);
            }
        }
        changed = true;
    }
    if let Some(r) = s.query_jobs.take() {
        match r {
            Ok(grid) => {
                s.results = Some(grid);
                s.results_table = TableState::new().key("results");
                s.query_error = None;
            }
            Err(e) => s.query_error = Some(e),
        }
        changed = true;
    }
    changed
}

fn on_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if shell::is_sidebar_toggle(ev) {
        s.sidebar.toggle();
        return true;
    }
    if ev.is_command('o') {
        App::choose_file(tx);
        return true;
    }
    if s.schema.is_none() {
        if ev.is_command('l') {
            s.path_edit.focused = true;
            return true;
        }
        return false;
    }
    let tab = match () {
        _ if ev.is_command('1') => Some(Tab::Data),
        _ if ev.is_command('2') => Some(Tab::Structure),
        _ if ev.is_command('3') => Some(Tab::Sql),
        _ => None,
    };
    if let Some(t) = tab {
        s.tab = t;
        if t == Tab::Sql {
            s.sql.focused = true;
        }
        return true;
    }
    if ev.is_command('r') {
        s.reload();
        return true;
    }
    if ev.is_command(']') {
        s.page_by(1);
        return true;
    }
    if ev.is_command('[') {
        s.page_by(-1);
        return true;
    }
    false
}

/// A small database for screenshots and first runs: `--scene demo`.
pub fn demo_path() -> PathBuf {
    let path = std::env::temp_dir().join("sift-demo.db");
    if !path.exists() {
        let c = rusqlite::Connection::open(&path).expect("demo db");
        c.execute_batch(include_str!("demo.sql")).expect("demo schema");
    }
    path
}

fn demo_scene(s: &mut App) {
    let schema = db::load_schema(demo_path()).expect("demo schema");
    s.path = Some(schema.path.clone());
    s.schema = Some(schema);
    s.selected = None;
    s.select(1);
    // Scenes render without workers: run the page now.
    let schema = s.schema.as_ref().unwrap();
    let obj = &schema.objects[1];
    s.grid = db::load_page(PageRequest { path: schema.path.clone(), table: obj.name.clone(), decls: obj.columns.iter().map(|c| c.decl.clone()).collect(), sort: None, offset: 0 }).ok();
    s.page_jobs.cancel();
}

fn demo_structure(s: &mut App) {
    demo_scene(s);
    s.tab = Tab::Structure;
}

fn demo_sql(s: &mut App) {
    demo_scene(s);
    s.tab = Tab::Sql;
    s.sql = TextEdit::new("SELECT c.name, count(*) AS orders, round(sum(o.total), 2) AS spent\nFROM customers c JOIN orders o ON o.customer_id = c.id\nGROUP BY c.id ORDER BY spent DESC;");
    s.results = db::run_query(s.path.clone().unwrap(), s.sql.text.clone()).ok();
}

fn welcome_scene(s: &mut App) {
    s.path = None;
    s.recents = vec![
        home::Recent { path: demo_path(), opened: home::now() - 300 },
        home::Recent { path: "/Users/example/Projects/inventory/inventory.sqlite".into(), opened: home::now() - 90_000 },
    ];
    s.found = Some(home::detect());
}

fn main() {
    rune::prewarm_gpu();
    // The Dock icon for `cargo run` too; a bundled Sift.app also carries it in Info.plist.
    let _ = rune::set_app_icon(include_bytes!("../assets/AppIcon.icns").to_vec());
    let spec = AppSpec {
        min_size: Some((720.0, 440.0)),
        scenes: &[("demo", demo_scene), ("structure", demo_structure), ("sql", demo_sql), ("welcome", welcome_scene)],
        families: ("Inter", "ui-monospace"),
        setup: Some(setup),
        poll: Some(poll),
        on_key: Some(on_key),
        ..AppSpec::new("Sift", (1120.0, 720.0), init, view::view)
    };
    let chrome = Chrome { full_size_content: true, traffic_lights: Some((16.0, 25.0)), drag_regions: true };
    rune::App::new(spec).chrome(chrome).run();
}
