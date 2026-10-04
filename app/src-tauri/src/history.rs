use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// One dictation, as it went to the target app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// Milliseconds since the Unix epoch, doubles as the id.
    pub at: u64,
    pub text: String,
    /// What the recognizer returned, before the text pipeline.
    pub raw: String,
    pub duration_ms: u64,
}

/// Append-only JSON lines: a dictation is on disk before any attempt to paste it, and a
/// crash can at worst cut the last line, which loading skips.
pub struct History {
    path: PathBuf,
}

impl History {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn append(&self, entry: &Entry) -> Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
        }
        let mut line = serde_json::to_string(entry)?;
        line.push('\n');
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("cannot open {}", self.path.display()))?;
        // A line torn by a crash would swallow this entry too: start on a fresh line.
        if !ends_with_newline(&mut file)? {
            line.insert(0, '\n');
        }
        file.write_all(line.as_bytes())?;
        file.sync_data()?;
        Ok(())
    }

    /// Newest first.
    pub fn load(&self) -> Vec<Entry> {
        let Ok(text) = fs::read_to_string(&self.path) else {
            return Vec::new();
        };
        let mut entries: Vec<Entry> = text
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        entries.reverse();
        entries
    }

    pub fn delete(&self, at: u64) -> Result<()> {
        self.rewrite(|e| e.at != at)
    }

    pub fn clear(&self) -> Result<()> {
        match fs::remove_file(&self.path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                Err(e).with_context(|| format!("cannot remove {}", self.path.display()))
            }
            _ => Ok(()),
        }
    }

    /// Drops entries older than `days` counted back from `now_ms`.
    pub fn prune(&self, days: u32, now_ms: u64) -> Result<()> {
        let cutoff = now_ms.saturating_sub(u64::from(days) * 24 * 60 * 60 * 1000);
        self.rewrite(|e| e.at >= cutoff)
    }

    fn rewrite(&self, keep: impl Fn(&Entry) -> bool) -> Result<()> {
        let mut entries = self.load();
        let before = entries.len();
        entries.retain(|e| keep(e));
        if entries.len() == before {
            return Ok(());
        }
        entries.reverse();
        let mut text = String::new();
        for entry in &entries {
            text.push_str(&serde_json::to_string(entry)?);
            text.push('\n');
        }
        let tmp = self.path.with_extension("jsonl.tmp");
        fs::write(&tmp, text).with_context(|| format!("cannot write {}", tmp.display()))?;
        fs::rename(&tmp, &self.path)
            .with_context(|| format!("cannot write {}", self.path.display()))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn ends_with_newline(file: &mut fs::File) -> std::io::Result<bool> {
    let len = file.metadata()?.len();
    if len == 0 {
        return Ok(true);
    }
    let mut last = [0u8];
    file.seek(SeekFrom::Start(len - 1))?;
    file.read_exact(&mut last)?;
    Ok(last[0] == b'\n')
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: u64 = 24 * 60 * 60 * 1000;

    fn entry(at: u64, text: &str) -> Entry {
        Entry {
            at,
            text: text.into(),
            raw: text.into(),
            duration_ms: 1200,
        }
    }

    fn history() -> (tempfile::TempDir, History) {
        let dir = tempfile::tempdir().unwrap();
        let history = History::new(dir.path().join("data/history.jsonl"));
        (dir, history)
    }

    #[test]
    fn empty_without_a_file() {
        let (_dir, history) = history();
        assert!(history.load().is_empty());
    }

    #[test]
    fn appends_and_loads_newest_first() {
        let (_dir, history) = history();
        history.append(&entry(1, "раз")).unwrap();
        history.append(&entry(2, "два\nс переносом")).unwrap();
        let loaded = history.load();
        assert_eq!(loaded, [entry(2, "два\nс переносом"), entry(1, "раз")]);
    }

    #[test]
    fn one_line_per_entry() {
        let (_dir, history) = history();
        history.append(&entry(1, "a\nb\nc")).unwrap();
        history.append(&entry(2, "d")).unwrap();
        let text = fs::read_to_string(history.path()).unwrap();
        assert_eq!(text.lines().count(), 2);
    }

    #[test]
    fn skips_a_torn_last_line() {
        let (_dir, history) = history();
        history.append(&entry(1, "целая")).unwrap();
        let mut file = OpenOptions::new()
            .append(true)
            .open(history.path())
            .unwrap();
        file.write_all(r#"{"at": 2, "text": "обрыв"#.as_bytes())
            .unwrap();
        assert_eq!(history.load(), [entry(1, "целая")]);
        history.append(&entry(3, "после")).unwrap();
        assert_eq!(history.load().len(), 2);
    }

    #[test]
    fn deletes_one() {
        let (_dir, history) = history();
        for at in 1..=3 {
            history.append(&entry(at, "x")).unwrap();
        }
        history.delete(2).unwrap();
        let ids: Vec<u64> = history.load().iter().map(|e| e.at).collect();
        assert_eq!(ids, [3, 1]);
    }

    #[test]
    fn deleting_a_missing_id_is_harmless() {
        let (_dir, history) = history();
        history.append(&entry(1, "x")).unwrap();
        history.delete(42).unwrap();
        assert_eq!(history.load().len(), 1);
    }

    #[test]
    fn clears() {
        let (_dir, history) = history();
        history.clear().unwrap();
        history.append(&entry(1, "x")).unwrap();
        history.clear().unwrap();
        assert!(history.load().is_empty());
    }

    #[test]
    fn prunes_old_entries() {
        let (_dir, history) = history();
        let now = 100 * DAY;
        history.append(&entry(now - 40 * DAY, "old")).unwrap();
        history.append(&entry(now - 29 * DAY, "recent")).unwrap();
        history.append(&entry(now, "now")).unwrap();
        history.prune(30, now).unwrap();
        let texts: Vec<String> = history.load().into_iter().map(|e| e.text).collect();
        assert_eq!(texts, ["now", "recent"]);
    }

    #[test]
    fn prune_keeps_order_for_later_appends() {
        let (_dir, history) = history();
        history.append(&entry(1, "old")).unwrap();
        history.append(&entry(10 * DAY, "a")).unwrap();
        history.prune(5, 11 * DAY).unwrap();
        history.append(&entry(11 * DAY, "b")).unwrap();
        let texts: Vec<String> = history.load().into_iter().map(|e| e.text).collect();
        assert_eq!(texts, ["b", "a"]);
    }
}
