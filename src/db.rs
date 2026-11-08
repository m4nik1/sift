//! Everything that touches SQLite. Each call opens its own read-only connection, so the work
//! can run on Rune's `Latest` workers without sharing a `Connection` across threads, and a
//! viewer can never write to the file it is showing.

use std::path::{Path, PathBuf};
use std::time::Instant;

use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags};

/// Rows fetched per page of a table, and the most a query returns.
pub const PAGE: usize = 1000;
pub const QUERY_LIMIT: usize = 5000;
/// Longest text a cell keeps; the rest is cut so a megabyte of JSON doesn't stall layout.
const CELL_CHARS: usize = 240;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjKind {
    Table,
    View,
}

#[derive(Clone, Debug)]
pub struct Column {
    pub name: String,
    /// The declared type, as written in the CREATE statement ("" when none).
    pub decl: String,
    pub pk: bool,
    pub not_null: bool,
}

#[derive(Clone, Debug)]
pub struct Object {
    pub name: String,
    pub kind: ObjKind,
    pub sql: String,
    pub columns: Vec<Column>,
    /// None when counting failed (a view over a missing table, say).
    pub rows: Option<u64>,
    pub indexes: Vec<(String, String)>,
}

#[derive(Clone, Debug)]
pub struct Schema {
    pub path: PathBuf,
    pub file_size: u64,
    pub sqlite_version: String,
    pub objects: Vec<Object>,
}

/// SQLite's five storage classes, which is what a cell is drawn by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Null,
    Integer,
    Real,
    Text,
    Blob,
}

#[derive(Clone, Debug)]
pub struct Value {
    pub class: Class,
    pub text: String,
}

#[derive(Clone, Debug, Default)]
pub struct Grid {
    pub columns: Vec<String>,
    /// Declared types, when the columns come straight from a table.
    pub decls: Vec<String>,
    pub rows: Vec<Vec<Value>>,
    /// Stable row ids for the table widget ("0", "1", …, offset by the page).
    pub ids: Vec<String>,
    /// Suggested column widths in points, from the header and the first rows.
    pub widths: Vec<f32>,
    /// Whether the first stored value of each column is a number (right-aligns it).
    pub numeric: Vec<bool>,
    pub offset: usize,
    /// More rows exist past this grid (queries stop at `QUERY_LIMIT`).
    pub truncated: bool,
    pub millis: f64,
}

fn open(path: &Path) -> Result<Connection, String> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_URI;
    let conn = Connection::open_with_flags(path, flags).map_err(|e| e.to_string())?;
    // Reject files that aren't databases now, with a clear message, rather than on first query.
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0))
        .map_err(|e| format!("{} doesn't look like a SQLite database ({e})", path.display()))?;
    Ok(conn)
}

pub fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

pub fn load_schema(path: PathBuf) -> Result<Schema, String> {
    let conn = open(&path)?;
    let sqlite_version: String = conn.query_row("SELECT sqlite_version()", [], |r| r.get(0)).unwrap_or_default();
    let file_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let mut stmt = conn
        .prepare("SELECT name, type, coalesce(sql, '') FROM sqlite_master WHERE type IN ('table','view') AND name NOT LIKE 'sqlite_%' ORDER BY type = 'view', name COLLATE NOCASE")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))
        .map_err(|e| e.to_string())?;
    let mut objects = Vec::new();
    for row in rows.flatten() {
        let (name, kind, sql) = row;
        let kind = if kind == "view" { ObjKind::View } else { ObjKind::Table };
        let columns = table_info(&conn, &name);
        let rows = conn.query_row(&format!("SELECT count(*) FROM {}", quote(&name)), [], |r| r.get::<_, i64>(0)).ok().map(|n| n as u64);
        let indexes = indexes(&conn, &name);
        objects.push(Object { name, kind, sql, columns, rows, indexes });
    }
    Ok(Schema { path, file_size, sqlite_version, objects })
}

fn table_info(conn: &Connection, table: &str) -> Vec<Column> {
    let Ok(mut stmt) = conn.prepare(&format!("PRAGMA table_info({})", quote(table))) else { return Vec::new() };
    stmt.query_map([], |r| {
        Ok(Column { name: r.get(1)?, decl: r.get::<_, Option<String>>(2)?.unwrap_or_default(), not_null: r.get::<_, i64>(3)? != 0, pk: r.get::<_, i64>(5)? != 0 })
    })
    .map(|it| it.flatten().collect())
    .unwrap_or_default()
}

fn indexes(conn: &Connection, table: &str) -> Vec<(String, String)> {
    let Ok(mut stmt) = conn.prepare("SELECT name, coalesce(sql, '') FROM sqlite_master WHERE type = 'index' AND tbl_name = ?1 ORDER BY name") else {
        return Vec::new();
    };
    stmt.query_map([table], |r| Ok((r.get(0)?, r.get(1)?))).map(|it| it.flatten().collect()).unwrap_or_default()
}

/// A page of a table or view, optionally ordered by one column.
pub struct PageRequest {
    pub path: PathBuf,
    pub table: String,
    pub decls: Vec<String>,
    pub sort: Option<(String, bool)>,
    pub offset: usize,
}

pub fn load_page(req: PageRequest) -> Result<Grid, String> {
    let conn = open(&req.path)?;
    let order = match &req.sort {
        Some((col, desc)) => format!(" ORDER BY {} {}", quote(col), if *desc { "DESC" } else { "ASC" }),
        None => String::new(),
    };
    let sql = format!("SELECT * FROM {}{order} LIMIT {} OFFSET {}", quote(&req.table), PAGE, req.offset);
    let mut grid = run(&conn, &sql, PAGE)?;
    grid.offset = req.offset;
    // Headers read "name · type", so make room for the type too.
    for (j, d) in req.decls.iter().enumerate().filter(|(_, d)| !d.is_empty()) {
        if let (Some(w), Some(name)) = (grid.widths.get_mut(j), grid.columns.get(j)) {
            *w = w.max(((name.chars().count() + d.chars().count() + 3) as f32 * 7.4 + 22.0).min(380.0));
        }
    }
    grid.decls = req.decls;
    grid.ids = (0..grid.rows.len()).map(|i| (req.offset + i).to_string()).collect();
    Ok(grid)
}

pub fn run_query(path: PathBuf, sql: String) -> Result<Grid, String> {
    let conn = open(&path)?;
    let sql = sql.trim().trim_end_matches(';').to_owned();
    if sql.is_empty() {
        return Err("Type a query, then press ⌘↩ to run it.".into());
    }
    run(&conn, &sql, QUERY_LIMIT)
}

fn run(conn: &Connection, sql: &str, limit: usize) -> Result<Grid, String> {
    let started = Instant::now();
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let columns: Vec<String> = stmt.column_names().into_iter().map(str::to_owned).collect();
    let n = columns.len();
    let mut rows = Vec::new();
    let mut truncated = false;
    let mut cursor = stmt.query([]).map_err(|e| e.to_string())?;
    while let Some(row) = cursor.next().map_err(|e| e.to_string())? {
        if rows.len() == limit {
            truncated = true;
            break;
        }
        rows.push((0..n).map(|i| value(row.get_ref(i).unwrap_or(ValueRef::Null))).collect::<Vec<_>>());
    }
    let ids = (0..rows.len()).map(|i| i.to_string()).collect();
    let widths = widths(&columns, &rows);
    let numeric = (0..n)
        .map(|j| rows.iter().map(|r: &Vec<Value>| r[j].class).find(|c| *c != Class::Null).is_some_and(|c| matches!(c, Class::Integer | Class::Real)))
        .collect();
    Ok(Grid { columns, decls: Vec::new(), rows, ids, widths, numeric, offset: 0, truncated, millis: started.elapsed().as_secs_f64() * 1000.0 })
}

fn value(v: ValueRef) -> Value {
    match v {
        ValueRef::Null => Value { class: Class::Null, text: "NULL".into() },
        ValueRef::Integer(i) => Value { class: Class::Integer, text: i.to_string() },
        ValueRef::Real(f) => Value { class: Class::Real, text: real(f) },
        ValueRef::Text(t) => Value { class: Class::Text, text: clip(&String::from_utf8_lossy(t)) },
        ValueRef::Blob(b) => Value { class: Class::Blob, text: blob(b) },
    }
}

fn real(f: f64) -> String {
    let s = format!("{f}");
    if s.contains('.') || s.contains('e') || s.contains("inf") || s.contains("NaN") { s } else { format!("{s}.0") }
}

/// One line, at most `CELL_CHARS` characters: newlines and tabs show as visible marks.
fn clip(s: &str) -> String {
    let mut out = String::with_capacity(s.len().min(CELL_CHARS * 4));
    for (i, ch) in s.chars().enumerate() {
        if i == CELL_CHARS {
            out.push('…');
            break;
        }
        out.push(match ch {
            '\n' => '↵',
            '\r' => ' ',
            '\t' => '⇥',
            c => c,
        });
    }
    out
}

fn blob(b: &[u8]) -> String {
    let head: String = b.iter().take(8).map(|x| format!("{x:02x}")).collect();
    let more = if b.len() > 8 { "…" } else { "" };
    format!("{} · {head}{more}", bytes(b.len() as u64))
}

pub fn bytes(n: u64) -> String {
    match n {
        n if n >= 1 << 30 => format!("{:.1} GB", n as f64 / (1u64 << 30) as f64),
        n if n >= 1 << 20 => format!("{:.1} MB", n as f64 / (1u64 << 20) as f64),
        n if n >= 1 << 10 => format!("{:.1} KB", n as f64 / 1024.0),
        n => format!("{n} B"),
    }
}

/// Width from the longest of the header and the first 200 values, at ~7 pt a character.
fn widths(columns: &[String], rows: &[Vec<Value>]) -> Vec<f32> {
    columns
        .iter()
        .enumerate()
        .map(|(j, name)| {
            let longest = rows.iter().take(200).map(|r| r[j].text.chars().count()).max().unwrap_or(0).max(name.chars().count() + 1);
            (longest as f32 * 7.4 + 22.0).clamp(64.0, 380.0)
        })
        .collect()
}

pub fn thousands(n: u64) -> String {
    let d = n.to_string();
    let mut out = String::new();
    for (i, ch) in d.chars().enumerate() {
        if i > 0 && (d.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sift-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.db");
        let _ = std::fs::remove_file(&path);
        let c = Connection::open(&path).unwrap();
        c.execute_batch(
            "CREATE TABLE \"we\"\"ird\" (id INTEGER PRIMARY KEY, name TEXT NOT NULL, score REAL, data BLOB);
             INSERT INTO \"we\"\"ird\" VALUES (1, 'a\nb', 1.5, x'00ff'), (2, 'c', NULL, NULL);
             CREATE VIEW v AS SELECT name FROM \"we\"\"ird\";",
        )
        .unwrap();
        path
    }

    #[test]
    fn schema_pages_and_queries() {
        let path = fixture();
        let s = load_schema(path.clone()).unwrap();
        assert_eq!(s.objects.len(), 2);
        let t = &s.objects[0];
        assert_eq!((t.name.as_str(), t.kind, t.rows), ("we\"ird", ObjKind::Table, Some(2)));
        assert!(t.columns[0].pk && t.columns[1].not_null);
        let g = load_page(PageRequest { path: path.clone(), table: t.name.clone(), decls: vec![], sort: Some(("id".into(), true)), offset: 0 }).unwrap();
        assert_eq!(g.rows[0][0].text, "2");
        assert_eq!(g.rows[1][1].text, "a↵b");
        assert_eq!(g.rows[1][3].class, Class::Blob);
        assert_eq!(g.rows[0][2].class, Class::Null);
        let q = run_query(path.clone(), "select count(*) as n from v;".into()).unwrap();
        assert_eq!(q.rows[0][0].text, "2");
        assert!(run_query(path.clone(), "delete from v".into()).is_err(), "read-only");
    }
}
