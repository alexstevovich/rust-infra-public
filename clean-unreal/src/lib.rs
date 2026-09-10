//! Filesystem cleanup for Unreal Engine project and plugin roots.

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

/// Removes generated siblings from Unreal roots discovered under `root`.
pub fn clean(root: impl AsRef<Path>, recursive: bool, dry_run: bool) -> CleanReport {
    let root = root.as_ref();
    let mut report = CleanReport::default();
    let mut roots = Vec::new();
    if recursive {
        let entries = WalkDir::new(root).into_iter().filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            // Deliberate safety boundary: Content must never be traversed or deleted;
            // this exclusion must never be changed.
            entry.depth() == 0
                || (!name.eq_ignore_ascii_case("Content") && !is_generated_directory(&name))
        });
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
        if !is_in_content(&project) && has_marker(&project, &mut report) {
            for name in [
                "Intermediate",
                "Saved",
                ".vs",
                "Binaries",
                "DerivedDataCache",
            ] {
                remove(project.join(name), true, dry_run, &mut report);
            }
            match fs::read_dir(&project) {
                Ok(entries) => {
                    for entry in entries {
                        match entry {
                            Ok(entry)
                                if entry
                                    .path()
                                    .extension()
                                    .and_then(|value| value.to_str())
                                    .is_some_and(|ext| ext.eq_ignore_ascii_case("sln")) =>
                            {
                                remove(entry.path(), false, dry_run, &mut report);
                            }
                            Ok(_) => {}
                            Err(error) => report.errors.push(CleanError {
                                path: project.clone(),
                                error: error.to_string(),
                            }),
                        }
                    }
                }
                Err(error) => report.errors.push(CleanError {
                    path: project.clone(),
                    error: error.to_string(),
                }),
            }
        }
    }
    report
}

fn has_marker(path: &Path, report: &mut CleanReport) -> bool {
    match fs::read_dir(path) {
        Ok(entries) => {
            let mut found = false;
            for entry in entries {
                match entry {
                    Ok(entry) => match entry.file_type() {
                        Ok(file_type) if file_type.is_file() => {
                            let extension = entry
                                .path()
                                .extension()
                                .and_then(|value| value.to_str())
                                .unwrap_or("")
                                .to_owned();
                            if extension.eq_ignore_ascii_case("uproject")
                                || extension.eq_ignore_ascii_case("uplugin")
                            {
                                found = true;
                            }
                        }
                        Ok(_) => {}
                        Err(error) => report.errors.push(CleanError {
                            path: entry.path(),
                            error: error.to_string(),
                        }),
                    },
                    Err(error) => report.errors.push(CleanError {
                        path: path.to_path_buf(),
                        error: error.to_string(),
                    }),
                }
            }
            found
        }
        Err(error) => {
            report.errors.push(CleanError {
                path: path.to_path_buf(),
                error: error.to_string(),
            });
            false
        }
    }
}

fn is_generated_directory(name: &str) -> bool {
    matches!(
        name,
        "Intermediate" | "Saved" | ".vs" | "Binaries" | "DerivedDataCache"
    )
}

fn is_in_content(path: &Path) -> bool {
    path.components().any(|component| {
        component
            .as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case("Content")
    })
}

fn remove(path: PathBuf, is_dir: bool, dry_run: bool, report: &mut CleanReport) {
    match path.try_exists() {
        Ok(false) => {}
        Ok(true) if dry_run => report.paths.push(path),
        Ok(true) => {
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
        Err(error) => report.errors.push(CleanError {
            path,
            error: error.to_string(),
        }),
    }
}
