use crate::{FileNode, FileTree, NodeKind, ScanStatus};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct ScanOptions {
    pub follow_symlinks: bool,
    pub max_depth: Option<usize>,
}

#[derive(Debug)]
pub enum ScanError {
    RootUnavailable { path: PathBuf, source: io::Error },
}

impl std::fmt::Display for ScanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RootUnavailable { path, source } => {
                write!(f, "could not scan root {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for ScanError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RootUnavailable { source, .. } => Some(source),
        }
    }
}

pub fn scan(root: impl AsRef<Path>, options: &ScanOptions) -> Result<FileTree, ScanError> {
    let root = root.as_ref();
    let metadata = fs::symlink_metadata(root).map_err(|source| ScanError::RootUnavailable {
        path: root.to_path_buf(),
        source,
    })?;

    Ok(FileTree {
        root: scan_node(root, &metadata, options, 0),
    })
}

fn scan_node(
    path: &Path,
    metadata: &fs::Metadata,
    options: &ScanOptions,
    depth: usize,
) -> FileNode {
    let file_type = metadata.file_type();
    let is_symlink = file_type.is_symlink();

    let effective_metadata = if is_symlink && options.follow_symlinks {
        fs::metadata(path).ok()
    } else {
        None
    };

    let kind_source = effective_metadata.as_ref().unwrap_or(metadata);
    let kind = if kind_source.is_dir() {
        NodeKind::Directory
    } else if kind_source.is_file() {
        NodeKind::File
    } else {
        NodeKind::Other
    };

    let size = match kind {
        NodeKind::File => Some(kind_source.len()),
        _ => None,
    };

    let mut node = FileNode {
        name: display_name(path),
        path: path.to_path_buf(),
        kind,
        size,
        is_symlink,
        status: ScanStatus::Complete,
        children: Vec::new(),
    };

    let depth_limited = options.max_depth.is_some_and(|max| depth >= max);
    let can_descend = kind == NodeKind::Directory && (!is_symlink || options.follow_symlinks);

    if can_descend && !depth_limited {
        match fs::read_dir(path) {
            Ok(entries) => {
                for entry_result in entries {
                    match entry_result {
                        Ok(entry) => {
                            let child_path = entry.path();
                            match fs::symlink_metadata(&child_path) {
                                Ok(child_metadata) => {
                                    node.children.push(scan_node(
                                        &child_path,
                                        &child_metadata,
                                        options,
                                        depth + 1,
                                    ));
                                }
                                Err(error) => node.children.push(error_node(child_path, error)),
                            }
                        }
                        Err(error) => {
                            node.status = status_from_io_error(error);
                        }
                    }
                }
                node.children.sort_by(|a, b| {
                    let a_dir = a.kind == NodeKind::Directory;
                    let b_dir = b.kind == NodeKind::Directory;
                    b_dir
                        .cmp(&a_dir)
                        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                });
            }
            Err(error) => {
                node.status = status_from_io_error(error);
            }
        }
    }

    node
}

fn error_node(path: PathBuf, error: io::Error) -> FileNode {
    FileNode {
        name: display_name(&path),
        path,
        kind: NodeKind::Other,
        size: None,
        is_symlink: false,
        status: status_from_io_error(error),
        children: Vec::new(),
    }
}

fn status_from_io_error(error: io::Error) -> ScanStatus {
    if error.kind() == io::ErrorKind::PermissionDenied {
        ScanStatus::AccessDenied
    } else {
        ScanStatus::Error(error.to_string())
    }
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| path.display().to_string())
}
