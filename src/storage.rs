use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::metadata;

#[derive(Debug, Clone)]
pub struct Storage {
    root: PathBuf,
}

impl Storage {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn default_root() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(metadata::APP_ID)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn settings_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(metadata::APP_ID)
            .join("settings.json")
    }

    pub fn migrate_legacy_app_dirs() -> Result<(), String> {
        let config_base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        let data_base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
        migrate_legacy_dirs(&config_base, &data_base)
    }

    pub fn studies_dir(&self) -> PathBuf {
        self.root.join("studies")
    }

    pub fn reviews_dir(&self) -> PathBuf {
        self.root.join("reviews")
    }

    pub fn ensure_base_dirs(&self) -> Result<(), String> {
        for dir in [self.root.clone(), self.studies_dir(), self.reviews_dir()] {
            fs::create_dir_all(&dir)
                .map_err(|e| format!("Failed to create {}: {e}", dir.display()))?;
        }
        Ok(())
    }
}

fn migrate_legacy_dirs(config_base: &Path, data_base: &Path) -> Result<(), String> {
    migrate_dir_if_needed(
        &config_base.join(metadata::LEGACY_APP_ID),
        &config_base.join(metadata::APP_ID),
    )?;
    migrate_dir_if_needed(
        &data_base.join(metadata::LEGACY_APP_ID),
        &data_base.join(metadata::APP_ID),
    )
}

fn migrate_dir_if_needed(legacy: &Path, current: &Path) -> Result<(), String> {
    if !legacy.exists() || current.exists() {
        return Ok(());
    }

    copy_dir_preserving_legacy(legacy, current).map_err(|e| {
        format!(
            "Failed to migrate {} to {}: {e}",
            legacy.display(),
            current.display()
        )
    })
}

fn copy_dir_preserving_legacy(source: &Path, dest: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dest)?;

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if dest_path.exists() {
            continue;
        }

        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir_preserving_legacy(&source_path, &dest_path)?;
        } else if file_type.is_file() {
            if fs::hard_link(&source_path, &dest_path).is_err() {
                fs::copy(&source_path, &dest_path)?;
            }
        } else if file_type.is_symlink() {
            copy_symlink(&source_path, &dest_path)?;
        }
    }

    Ok(())
}

#[cfg(unix)]
fn copy_symlink(source: &Path, dest: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(fs::read_link(source)?, dest)
}

#[cfg(not(unix))]
fn copy_symlink(source: &Path, dest: &Path) -> std::io::Result<()> {
    let target = fs::read_link(source)?;
    let target = if target.is_absolute() {
        target
    } else {
        source
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(target)
    };

    if target.is_dir() {
        copy_dir_preserving_legacy(&target, dest)
    } else {
        fs::copy(&target, dest).map(|_| ())
    }
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    ensure_parent_dir(path)?;

    let tmp_path = atomic_temp_path(path);

    fs::write(&tmp_path, bytes)
        .map_err(|e| format!("Failed to write {}: {e}", tmp_path.display()))?;

    if let Err(e) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!("Failed to replace {}: {e}", path.display()));
    }

    Ok(())
}

pub fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create {}: {e}", parent.display()))?;
    }
    Ok(())
}

pub fn sanitize_filename(name: &str) -> String {
    let sanitized = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim()
        .to_lowercase()
        .replace(' ', "_");

    if sanitized.is_empty() {
        "untitled".to_string()
    } else {
        sanitized
    }
}

pub fn atomic_temp_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    path.with_file_name(format!(".{file_name}.{}.{}.tmp", std::process::id(), nanos))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_filename_keeps_readable_safe_names() {
        assert_eq!(
            sanitize_filename("King's Gambit Study"),
            "king_s_gambit_study"
        );
        assert_eq!(sanitize_filename("A-B_C 42"), "a-b_c_42");
    }

    #[test]
    fn sanitize_filename_uses_fallback_for_blank_names() {
        assert_eq!(sanitize_filename("   "), "untitled");
        assert_eq!(sanitize_filename(""), "untitled");
    }

    #[test]
    fn migration_preserves_legacy_dirs_and_copies_missing_current_dirs() {
        let root = temp_test_root("migration_copies");
        let config_base = root.join("config");
        let data_base = root.join("data");
        let legacy_config = config_base.join(metadata::LEGACY_APP_ID);
        let legacy_data = data_base.join(metadata::LEGACY_APP_ID);

        fs::create_dir_all(&legacy_config).unwrap();
        fs::create_dir_all(legacy_data.join("studies")).unwrap();
        fs::write(legacy_config.join("settings.json"), br#"{"ui_scale":1.0}"#).unwrap();
        fs::write(legacy_data.join("studies").join("line.pgn"), b"1. e4").unwrap();

        migrate_legacy_dirs(&config_base, &data_base).unwrap();

        assert!(legacy_config.join("settings.json").exists());
        assert!(legacy_data.join("studies").join("line.pgn").exists());
        assert!(
            config_base
                .join(metadata::APP_ID)
                .join("settings.json")
                .exists()
        );
        assert!(
            data_base
                .join(metadata::APP_ID)
                .join("studies")
                .join("line.pgn")
                .exists()
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn migration_does_not_overwrite_existing_current_dirs() {
        let root = temp_test_root("migration_keeps_current");
        let config_base = root.join("config");
        let data_base = root.join("data");
        let legacy_config = config_base.join(metadata::LEGACY_APP_ID);
        let current_config = config_base.join(metadata::APP_ID);

        fs::create_dir_all(&legacy_config).unwrap();
        fs::create_dir_all(&current_config).unwrap();
        fs::write(legacy_config.join("settings.json"), b"legacy").unwrap();
        fs::write(current_config.join("settings.json"), b"current").unwrap();

        migrate_legacy_dirs(&config_base, &data_base).unwrap();

        assert_eq!(
            fs::read_to_string(current_config.join("settings.json")).unwrap(),
            "current"
        );

        let _ = fs::remove_dir_all(root);
    }

    fn temp_test_root(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("tactica-{name}-{}-{nanos}", std::process::id()))
    }
}
