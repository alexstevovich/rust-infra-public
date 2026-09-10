use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileTree {
    pub root: FileNode,
}

impl FileTree {
    pub fn truncate_to_depth(&mut self, max_depth: usize) {
        self.root.truncate_to_depth(max_depth);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileNode {
    pub name: String,
    pub path: PathBuf,
    pub kind: NodeKind,
    pub size: Option<u64>,
    pub is_symlink: bool,
    pub status: ScanStatus,
    pub children: Vec<FileNode>,
}

impl FileNode {
    fn truncate_to_depth(&mut self, remaining: usize) {
        if remaining == 0 {
            self.children.clear();
        } else {
            for child in &mut self.children {
                child.truncate_to_depth(remaining - 1);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    File,
    Directory,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScanStatus {
    Complete,
    AccessDenied,
    Error(String),
}
