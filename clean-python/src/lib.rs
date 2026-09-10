//! Filesystem cleanup for Python build products, caches, and bytecode.

use std::{
    fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanError {
    pub path: PathBuf,
    pub error: String,
}

#[derive(Debug, Default)]
pub struct CleanReport {
    pub paths: Vec<PathBuf>,
    pub errors: Vec<CleanError>,
}

/// Removes Python artifacts under `root`; virtual environments require `include_venv`.
pub fn clean(
    root: impl AsRef<Path>,
    recursive: bool,
    dry_run: bool,
    include_venv: bool,
) -> CleanReport {
    let root = root.as_ref();
    let mut report = CleanReport::default();
    if recursive {
        let mut entries = WalkDir::new(root).into_iter();
        while let Some(entry) = entries.next() {
            match entry {
                Ok(entry) if entry.path() != root && is_artifact(&entry, include_venv) => {
                    let is_dir = entry.file_type().is_dir();
                    let path = entry.into_path();
                    if is_dir {
                        entries.skip_current_dir();
                    }
                    remove(path, is_dir, dry_run, &mut report);
                }
                Ok(_) => {}
                Err(error) => report.errors.push(CleanError {
                    path: error.path().unwrap_or(root).to_path_buf(),
                    error: error.to_string(),
                }),
            }
        }
    } else {
        match fs::read_dir(root) {
            Ok(entries) => {
                for entry in entries {
                    match entry {
                        Ok(entry) => match entry.file_type() {
                            Ok(file_type) => {
                                let path = entry.path();
                                if is_artifact_path(&path, file_type.is_dir(), include_venv) {
                                    remove(path, file_type.is_dir(), dry_run, &mut report);
                                }
                            }
                            Err(error) => report.errors.push(CleanError {
                                path: entry.path(),
                                error: error.to_string(),
                            }),
                        },
                        Err(error) => report.errors.push(CleanError {
                            path: root.to_path_buf(),
                            error: error.to_string(),
                        }),
                    }
                }
            }
            Err(error) => report.errors.push(CleanError {
                path: root.to_path_buf(),
                error: error.to_string(),
            }),
        }
    }
    report
}

fn is_artifact(entry: &walkdir::DirEntry, include_venv: bool) -> bool {
    is_artifact_path(entry.path(), entry.file_type().is_dir(), include_venv)
}

fn is_artifact_path(path: &Path, is_dir: bool, include_venv: bool) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if is_dir {
        matches!(
            name,
            "__pycache__" | "build" | "dist" | ".pytest_cache" | ".mypy_cache" | ".ruff_cache"
        ) || name.ends_with(".egg-info")
            || (include_venv && name == ".venv")
    } else {
        name.ends_with(".pyc") || name.ends_with(".pyo")
    }
}

fn remove(path: PathBuf, is_dir: bool, dry_run: bool, report: &mut CleanReport) {
    if dry_run {
        report.paths.push(path);
        return;
    }
    let result = if is_dir {
        fs::remove_dir_all(&path)
    } else {
        fs::remove_file(&path)
    };
    match result {
        Ok(()) => report.paths.push(path),
        Err(error) => report.errors.push(CleanError {
            path,
            error: error.to_string(),
        }),
    }
}
