//! Library management for study and review files.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::core::config;
use crate::storage::write_atomic;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryKind {
    Study,
    Review,
}

#[derive(Debug, Clone)]
pub struct Library {
    pub root: PathBuf,
    pub entries: Vec<LibraryEntry>,
    pub expanded_folders: HashSet<PathBuf>,
    pub recent: Vec<PathBuf>,
    meta: LibraryMeta,
}

#[derive(Debug, Clone)]
pub enum LibraryEntry {
    Folder {
        name: String,
        path: PathBuf,
        children: Vec<LibraryEntry>,
    },
    File {
        name: String,
        path: PathBuf,
        modified: Option<SystemTime>,
        kind: EntryKind,
        favorite: bool,
    },
}

impl LibraryEntry {
    pub fn name(&self) -> &str {
        match self {
            LibraryEntry::Folder { name, .. } => name,
            LibraryEntry::File { name, .. } => name,
        }
    }

    pub fn path(&self) -> &Path {
        match self {
            LibraryEntry::Folder { path, .. } => path,
            LibraryEntry::File { path, .. } => path,
        }
    }

    pub fn is_folder(&self) -> bool {
        matches!(self, LibraryEntry::Folder { .. })
    }

    pub fn kind(&self) -> Option<EntryKind> {
        match self {
            LibraryEntry::Folder { .. } => None,
            LibraryEntry::File { kind, .. } => Some(*kind),
        }
    }

    pub fn favorite(&self) -> bool {
        match self {
            LibraryEntry::Folder { .. } => false,
            LibraryEntry::File { favorite, .. } => *favorite,
        }
    }
}

impl Library {
    pub fn new() -> Self {
        let root = config::data_dir();
        let _ = fs::create_dir_all(config::studies_dir());
        let _ = fs::create_dir_all(config::reviews_dir());

        let mut lib = Self {
            root,
            entries: Vec::new(),
            expanded_folders: HashSet::new(),
            recent: Vec::new(),
            meta: LibraryMeta::default(),
        };

        lib.refresh();
        lib
    }

    pub fn refresh(&mut self) {
        self.meta = self.load_meta();
        let studies_dir = config::studies_dir();
        let reviews_dir = config::reviews_dir();

        self.entries = vec![
            LibraryEntry::Folder {
                name: "Studies".to_string(),
                path: studies_dir.clone(),
                children: scan_directory(&studies_dir, EntryKind::Study, &self.meta),
            },
            LibraryEntry::Folder {
                name: "Reviews".to_string(),
                path: reviews_dir.clone(),
                children: scan_directory(&reviews_dir, EntryKind::Review, &self.meta),
            },
        ];
    }

    pub fn toggle_folder(&mut self, path: &Path) {
        if self.expanded_folders.contains(path) {
            self.expanded_folders.remove(path);
        } else {
            self.expanded_folders.insert(path.to_path_buf());
        }
    }

    pub fn is_expanded(&self, path: &Path) -> bool {
        self.expanded_folders.contains(path)
    }

    pub fn search(
        &self,
        query: &str,
        kind_filter: Option<EntryKind>,
        favorites_only: bool,
    ) -> Vec<&LibraryEntry> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();
        search_entries(
            &self.entries,
            &query_lower,
            kind_filter,
            favorites_only,
            &mut results,
        );
        results
    }

    pub fn create_folder(&mut self, name: &str) -> Result<PathBuf, String> {
        let path = config::studies_dir().join(name);
        fs::create_dir_all(&path).map_err(|e| format!("Failed to create folder: {e}"))?;
        self.refresh();
        Ok(path)
    }

    pub fn toggle_favorite(&mut self, path: &Path) -> Result<bool, String> {
        let key = self.path_key(path);
        let current = self
            .meta
            .files
            .get(&key)
            .map(|m| m.favorite)
            .unwrap_or(false);
        let next = !current;

        self.meta
            .files
            .entry(key)
            .and_modify(|m| m.favorite = next)
            .or_insert(LibraryItemMeta { favorite: next });

        self.save_meta()?;
        self.refresh();
        Ok(next)
    }

    pub fn is_favorite(&self, path: &Path) -> bool {
        let key = self.path_key(path);
        self.meta
            .files
            .get(&key)
            .map(|m| m.favorite)
            .unwrap_or(false)
    }

    pub fn kind_for_path(&self, path: &Path) -> Option<EntryKind> {
        if path.starts_with(config::studies_dir()) {
            Some(EntryKind::Study)
        } else if path.starts_with(config::reviews_dir()) {
            Some(EntryKind::Review)
        } else {
            None
        }
    }

    pub fn delete(&mut self, path: &Path) -> Result<(), String> {
        if path.is_dir() {
            fs::remove_dir_all(path).map_err(|e| format!("Failed to delete folder: {e}"))?;
        } else {
            fs::remove_file(path).map_err(|e| format!("Failed to delete file: {e}"))?;
            if self.kind_for_path(path) == Some(EntryKind::Review) {
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("review");
                let sidecar = path.with_file_name(format!("{}.review.json", stem));
                if sidecar.exists() {
                    fs::remove_file(&sidecar)
                        .map_err(|e| format!("Failed to delete review sidecar: {e}"))?;
                }
            }
        }

        self.recent.retain(|p| p != path);
        self.meta.files.remove(&self.path_key(path));
        self.save_meta()?;
        self.refresh();
        Ok(())
    }

    fn meta_path(&self) -> PathBuf {
        self.root.join("library_meta.json")
    }

    fn path_key(&self, path: &Path) -> String {
        path.to_string_lossy().to_string()
    }

    fn load_meta(&self) -> LibraryMeta {
        let path = self.meta_path();
        let Ok(content) = fs::read_to_string(path) else {
            return LibraryMeta::default();
        };
        serde_json::from_str(&content).unwrap_or_default()
    }

    fn save_meta(&self) -> Result<(), String> {
        let path = self.meta_path();
        let json = serde_json::to_string_pretty(&self.meta)
            .map_err(|e| format!("Failed to serialize metadata: {e}"))?;
        write_atomic(&path, json.as_bytes()).map_err(|e| format!("Failed to write metadata: {e}"))
    }
}

impl Default for Library {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct LibraryMeta {
    #[serde(default)]
    files: HashMap<String, LibraryItemMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LibraryItemMeta {
    #[serde(default)]
    favorite: bool,
}

fn scan_directory(path: &Path, kind: EntryKind, meta: &LibraryMeta) -> Vec<LibraryEntry> {
    let mut entries = Vec::new();

    let Ok(read_dir) = fs::read_dir(path) else {
        return entries;
    };

    let mut items: Vec<_> = read_dir.filter_map(|e| e.ok()).collect();
    items.sort_by(|a, b| {
        let a_is_dir = a.path().is_dir();
        let b_is_dir = b.path().is_dir();

        match (a_is_dir, b_is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.file_name().cmp(&b.file_name()),
        }
    });

    for item in items {
        let item_path = item.path();
        let name = item_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unnamed")
            .to_string();

        if item_path.is_dir() {
            if name.starts_with('.') {
                continue;
            }

            entries.push(LibraryEntry::Folder {
                name,
                path: item_path.clone(),
                children: scan_directory(&item_path, kind, meta),
            });
        } else {
            let ext = item_path
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_lowercase());

            if ext.as_deref() != Some("pgn") {
                continue;
            }

            let modified = item.metadata().ok().and_then(|m| m.modified().ok());
            let favorite = meta
                .files
                .get(&item_path.to_string_lossy().to_string())
                .map(|m| m.favorite)
                .unwrap_or(false);

            entries.push(LibraryEntry::File {
                name,
                path: item_path,
                modified,
                kind,
                favorite,
            });
        }
    }

    entries
}

fn search_entries<'a>(
    entries: &'a [LibraryEntry],
    query: &str,
    kind_filter: Option<EntryKind>,
    favorites_only: bool,
    results: &mut Vec<&'a LibraryEntry>,
) {
    for entry in entries {
        if entry.name().to_lowercase().contains(query) {
            match entry {
                LibraryEntry::Folder { .. } => results.push(entry),
                LibraryEntry::File { kind, favorite, .. } => {
                    let kind_ok = kind_filter.is_none_or(|k| *kind == k);
                    let favorite_ok = !favorites_only || *favorite;
                    if kind_ok && favorite_ok {
                        results.push(entry);
                    }
                }
            }
        }

        if let LibraryEntry::Folder { children, .. } = entry {
            search_entries(children, query, kind_filter, favorites_only, results);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_entry_methods() {
        let file = LibraryEntry::File {
            name: "test".to_string(),
            path: PathBuf::from("/test/path.pgn"),
            modified: None,
            kind: EntryKind::Study,
            favorite: true,
        };

        assert_eq!(file.name(), "test");
        assert_eq!(file.path(), Path::new("/test/path.pgn"));
        assert!(!file.is_folder());
        assert_eq!(file.kind(), Some(EntryKind::Study));
        assert!(file.favorite());
    }
}
