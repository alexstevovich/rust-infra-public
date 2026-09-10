//! Filesystem cleanup for Rust projects identified by `Cargo.toml`.

use std::{
    fs, io,
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

/// Removes `target` from the root Rust project, or all discovered projects when recursive.
pub fn clean(root: impl AsRef<Path>, recursive: bool, dry_run: bool) -> CleanReport {
    let root = root.as_ref();
    let mut report = CleanReport::default();
    let mut roots = Vec::new();
    if recursive {
        let entries = WalkDir::new(root)
            .into_iter()
            .filter_entry(|entry| entry.file_name() != "target");
        for entry in entries {
            match entry {
                Ok(entry) if entry.file_type().is_dir() => roots.push(entry.into_path()),
                Ok(_) => {}
                Err(error) => report.errors.push(CleanError {
                    path: error.path().unwrap_or(root).to_path_buf(),
                    error: error.to_string(),
                }),
            }
        }
    } else {
        roots.push(root.to_path_buf());
    }

    for project in roots {
        let marker = project.join("Cargo.toml");
        match fs::metadata(&marker) {
            Ok(metadata) if metadata.is_file() => {
                remove_directory(project.join("target"), dry_run, &mut report)
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => push_error(marker, error, &mut report),
        }
    }
    report
}

fn remove_directory(path: PathBuf, dry_run: bool, report: &mut CleanReport) {
    match path.try_exists() {
        Ok(false) => {}
        Ok(true) if dry_run => report.paths.push(path),
        Ok(true) => match fs::remove_dir_all(&path) {
            Ok(()) => report.paths.push(path),
            Err(error) => push_error(path, error, report),
        },
        Err(error) => push_error(path, error, report),
    }
}

fn push_error(path: PathBuf, error: io::Error, report: &mut CleanReport) {
    report.errors.push(CleanError {
        path,
        error: error.to_string(),
    });
}
