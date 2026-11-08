//! The start screen's data: recently opened databases (kept on disk) and the SQLite files
//! other running apps have open right now (found with `lsof`, by file name only, so nothing
//! is read from another app's files until the user opens one).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::DB_EXTENSIONS;

const MAX_RECENTS: usize = 12;

#[derive(Clone, Debug, PartialEq)]
pub struct Recent {
    pub path: PathBuf,
    /// Seconds since the Unix epoch.
    pub opened: u64,
}

fn recents_file() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Application Support/Sift/recents.txt"))
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Reads the recents list: one `seconds<TAB>path` per line, newest first.
pub fn load_recents() -> Vec<Recent> {
    let Some(file) = recents_file() else { return Vec::new() };
    let Ok(text) = std::fs::read_to_string(file) else { return Vec::new() };
    parse_recents(&text)
}

fn parse_recents(text: &str) -> Vec<Recent> {
    text.lines()
        .filter_map(|line| {
            let (secs, path) = line.split_once('\t')?;
            Some(Recent { path: PathBuf::from(path), opened: secs.parse().ok()? })
        })
        .take(MAX_RECENTS)
        .collect()
}

/// Moves `path` to the top of `recents` and saves the list. Saving is best effort: a failure
/// only means the list isn't remembered next launch.
pub fn remember(recents: &mut Vec<Recent>, path: &Path) {
    recents.retain(|r| r.path != path);
    recents.insert(0, Recent { path: path.to_path_buf(), opened: now() });
    recents.truncate(MAX_RECENTS);
    save(recents);
}

pub fn forget(recents: &mut Vec<Recent>, path: &Path) {
    recents.retain(|r| r.path != path);
    save(recents);
}

pub fn save(recents: &[Recent]) {
    let Some(file) = recents_file() else { return };
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let text: String = recents.iter().map(|r| format!("{}\t{}\n", r.opened, r.path.display())).collect();
    let _ = std::fs::write(file, text);
}

/// "just now", "5 minutes ago", "yesterday", "12 Mar"-style coarse ages.
pub fn ago(secs: u64) -> String {
    let d = now().saturating_sub(secs);
    match d {
        0..60 => "just now".into(),
        60..3600 => plural(d / 60, "minute"),
        3600..86_400 => plural(d / 3600, "hour"),
        86_400..172_800 => "yesterday".into(),
        172_800..2_592_000 => plural(d / 86_400, "day"),
        2_592_000..31_536_000 => plural(d / 2_592_000, "month"),
        _ => plural(d / 31_536_000, "year"),
    }
}

fn plural(n: u64, unit: &str) -> String {
    format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" })
}

/// `~/x/y` for paths in the home folder.
pub fn tilde(path: &Path) -> String {
    if let Some(home) = std::env::var_os("HOME") {
        if let Ok(rest) = path.strip_prefix(&home) {
            return format!("~/{}", rest.display());
        }
    }
    path.display().to_string()
}

/// A typed path: trims spaces and quotes (Finder's "Copy as Pathname" and drag-into-Terminal
/// add them), unescapes `\ `, and expands `~`.
pub fn typed_path(input: &str) -> Option<PathBuf> {
    let s = input.trim().trim_matches(|c| c == '"' || c == '\'').trim().replace("\\ ", " ");
    if s.is_empty() {
        return None;
    }
    if let Some(rest) = s.strip_prefix("~/").or(if s == "~" { Some("") } else { None }) {
        let home = std::env::var_os("HOME")?;
        return Some(PathBuf::from(home).join(rest));
    }
    Some(PathBuf::from(s))
}

#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub path: PathBuf,
    /// The apps that have it open, sorted and without repeats.
    pub apps: Vec<String>,
}

/// The SQLite databases this user's other processes have open. A file counts when its name
/// has a database extension, or when a `-wal`, `-shm` or `-journal` companion is open (then
/// the database is the name without that suffix, which is how Chrome's `History` shows up).
pub fn detect() -> Result<Vec<Found>, String> {
    let user = std::env::var("USER").map_err(|_| "Can't tell which user is logged in.".to_owned())?;
    let out = Command::new("/usr/sbin/lsof")
        .args(["-n", "-P", "-w", "+c", "0", "-u", &user, "-F", "pcftn"])
        .output()
        .map_err(|e| format!("Couldn't run lsof: {e}"))?;
    Ok(parse_lsof(&String::from_utf8_lossy(&out.stdout), std::process::id()))
}

fn parse_lsof(out: &str, own_pid: u32) -> Vec<Found> {
    let mut found: BTreeMap<PathBuf, Vec<String>> = BTreeMap::new();
    let (mut pid, mut cmd, mut regular) = (0u32, String::new(), false);
    for line in out.lines() {
        let (tag, rest) = line.split_at(line.len().min(1));
        match tag {
            "p" => pid = rest.parse().unwrap_or(0),
            "c" => cmd = rest.to_owned(),
            "f" => regular = false,
            "t" => regular = rest == "REG",
            "n" if regular && pid != own_pid => {
                if let Some(db) = database_for(rest) {
                    let apps = found.entry(db).or_default();
                    if !apps.contains(&cmd) {
                        apps.push(cmd.clone());
                    }
                }
            }
            _ => {}
        }
    }
    let mut list: Vec<Found> = found
        .into_iter()
        .filter(|(p, _)| p.is_file())
        .map(|(path, mut apps)| {
            apps.sort_by_key(|a| a.to_lowercase());
            Found { path, apps }
        })
        .collect();
    // Things the user made (outside ~/Library and system folders) first, then by app.
    list.sort_by_key(|f| (is_system(&f.path), f.apps[0].to_lowercase(), f.path.clone()));
    list
}

fn database_for(name: &str) -> Option<PathBuf> {
    for suffix in ["-wal", "-shm", "-journal"] {
        if let Some(main) = name.strip_suffix(suffix) {
            return Some(PathBuf::from(main));
        }
    }
    let ext = Path::new(name).extension()?.to_str()?.to_ascii_lowercase();
    DB_EXTENSIONS.contains(&ext.as_str()).then(|| PathBuf::from(name))
}

pub fn is_system(path: &Path) -> bool {
    let s = path.to_string_lossy();
    let home_lib = std::env::var("HOME").map(|h| format!("{h}/Library/")).unwrap_or_default();
    s.starts_with("/System/") || s.starts_with("/Library/") || s.starts_with("/private/") || s.starts_with("/var/") || (!home_lib.is_empty() && s.starts_with(&home_lib))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsof_lines_become_databases_with_their_apps() {
        let dir = std::env::temp_dir().join(format!("sift-home-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("notes.sqlite");
        let b = dir.join("History");
        std::fs::write(&a, b"x").unwrap();
        std::fs::write(&b, b"x").unwrap();
        let out = format!(
            "p10\ncNotes\nf4\ntREG\nn{a}\nf5\ntREG\nn{a}-wal\nf6\ntDIR\nn{dir}\np11\ncHelium\nf7\ntREG\nn{b}-journal\nf8\ntREG\nn{a}\np12\ncSift\nf3\ntREG\nn{a}\n",
            a = a.display(),
            b = b.display(),
            dir = dir.display()
        );
        let found = parse_lsof(&out, 12);
        let get = |p: &Path| found.iter().find(|f| f.path == p).map(|f| f.apps.clone());
        assert_eq!(get(&a), Some(vec!["Helium".to_owned(), "Notes".to_owned()]));
        assert_eq!(get(&b), Some(vec!["Helium".to_owned()]));
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn typed_paths_and_recents_round_trip() {
        let home = std::env::var("HOME").unwrap();
        assert_eq!(typed_path(" '~/a\\ b.db' "), Some(PathBuf::from(format!("{home}/a b.db"))));
        assert_eq!(typed_path("\"/x/y.db\""), Some(PathBuf::from("/x/y.db")));
        assert_eq!(typed_path("  "), None);
        let r = parse_recents("5\t/a.db\nbad\n7\t/b c.db\n");
        assert_eq!(r, vec![Recent { path: "/a.db".into(), opened: 5 }, Recent { path: "/b c.db".into(), opened: 7 }]);
    }
}
