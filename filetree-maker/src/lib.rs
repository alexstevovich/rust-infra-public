mod elevate;
mod model;
#[cfg(windows)]
mod ntfs;
mod scanner;

pub use elevate::{
    ElevationError, is_elevated_scan_command, request_elevated_scan, run_elevated_scan,
};
pub use model::{FileNode, FileTree, NodeKind, ScanStatus};
pub use scanner::{ScanError, ScanOptions, scan};
