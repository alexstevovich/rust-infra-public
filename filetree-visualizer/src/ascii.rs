use crate::{Visibility, VisibilityRules};
use filetree_maker::{FileNode, FileTree, NodeKind, ScanStatus};

const FLOW_STYLE: &str = "\x1b[1;38;2;204;170;255m";
const NODE_STYLE: &str = "\x1b[1;38;2;255;255;255m";
const COLLAPSED_STYLE: &str = "\x1b[1;38;2;255;213;128m";
const RESET_STYLE: &str = "\x1b[0m";

#[derive(Debug, Clone)]
pub struct AsciiOptions {
    pub show_sizes: bool,
    pub mark_symlinks: bool,
    pub rules: VisibilityRules,
}

impl Default for AsciiOptions {
    fn default() -> Self {
        Self {
            show_sizes: false,
            mark_symlinks: true,
            rules: VisibilityRules::default(),
        }
    }
}

pub fn render_ascii(tree: &FileTree, options: &AsciiOptions) -> String {
    let mut out = String::new();
    out.push_str(NODE_STYLE);
    out.push_str(&format_node(&tree.root, options, false));
    out.push_str(RESET_STYLE);
    out.push('\n');
    render_children(&tree.root, options, "", &mut out);
    out
}

fn render_children(node: &FileNode, options: &AsciiOptions, prefix: &str, out: &mut String) {
    let visible: Vec<&FileNode> = node
        .children
        .iter()
        .filter(|child| node_visibility(child, options) != Visibility::Omit)
        .collect();

    for (index, child) in visible.iter().enumerate() {
        let last = index + 1 == visible.len();
        out.push_str(FLOW_STYLE);
        out.push_str(prefix);
        out.push_str(if last { "└── " } else { "├── " });
        out.push_str(RESET_STYLE);

        let visibility = node_visibility(child, options);
        out.push_str(if visibility == Visibility::Collapse {
            COLLAPSED_STYLE
        } else {
            NODE_STYLE
        });
        out.push_str(&format_node(
            child,
            options,
            visibility == Visibility::Collapse,
        ));
        out.push_str(RESET_STYLE);
        out.push('\n');

        if child.kind == NodeKind::Directory && visibility == Visibility::Normal {
            let child_prefix = format!("{}{}", prefix, if last { "    " } else { "│   " });
            render_children(child, options, &child_prefix, out);
        }
    }
}

fn node_visibility(node: &FileNode, options: &AsciiOptions) -> Visibility {
    if node.kind == NodeKind::Directory {
        options.rules.visibility_for(&node.name)
    } else {
        Visibility::Normal
    }
}

fn format_node(node: &FileNode, options: &AsciiOptions, collapsed: bool) -> String {
    let mut label = node.name.clone();

    if node.kind == NodeKind::Directory && !(label.ends_with('/') || label.ends_with('\\')) {
        label.push('/');
    }

    if options.mark_symlinks && node.is_symlink {
        label.push_str(" -> [symlink]");
    }

    match &node.status {
        ScanStatus::Complete => {}
        ScanStatus::AccessDenied => label.push_str(" [access denied]"),
        ScanStatus::Error(message) => {
            label.push_str(" [error: ");
            label.push_str(message);
            label.push(']');
        }
    }

    if collapsed {
        label.push_str(" [...]");
    }

    if options.show_sizes
        && let Some(size) = node.size
    {
        label.push_str(&format!(" ({size} B)"));
    }

    label
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PatternRule, Visibility};
    use std::path::PathBuf;

    fn node(name: &str, kind: NodeKind, children: Vec<FileNode>) -> FileNode {
        FileNode {
            name: name.to_string(),
            path: PathBuf::from(name),
            kind,
            size: None,
            is_symlink: false,
            status: ScanStatus::Complete,
            children,
        }
    }

    #[test]
    fn renders_hierarchy() {
        let tree = FileTree {
            root: node(
                "project",
                NodeKind::Directory,
                vec![node(
                    "src",
                    NodeKind::Directory,
                    vec![node("main.rs", NodeKind::File, vec![])],
                )],
            ),
        };

        assert_eq!(
            render_ascii(&tree, &AsciiOptions::default()),
            "\x1b[1;38;2;255;255;255mproject/\x1b[0m\n\
             \x1b[1;38;2;204;170;255m└── \x1b[0m\x1b[1;38;2;255;255;255msrc/\x1b[0m\n\
             \x1b[1;38;2;204;170;255m    └── \x1b[0m\x1b[1;38;2;255;255;255mmain.rs\x1b[0m\n"
        );
    }

    #[test]
    fn collapses_matching_node() {
        let tree = FileTree {
            root: node(
                "project",
                NodeKind::Directory,
                vec![node(
                    "target",
                    NodeKind::Directory,
                    vec![node("artifact.bin", NodeKind::File, vec![])],
                )],
            ),
        };

        let options = AsciiOptions {
            rules: VisibilityRules {
                rules: vec![PatternRule {
                    pattern: "target".into(),
                    visibility: Visibility::Collapse,
                }],
            },
            ..AsciiOptions::default()
        };

        assert_eq!(
            render_ascii(&tree, &options),
            "\x1b[1;38;2;255;255;255mproject/\x1b[0m\n\
             \x1b[1;38;2;204;170;255m└── \x1b[0m\x1b[1;38;2;255;213;128mtarget/ [...]\x1b[0m\n"
        );
    }

    #[test]
    fn repeated_collapse_rules_match_exact_directory_names() {
        let tree = FileTree {
            root: node(
                "project",
                NodeKind::Directory,
                vec![
                    node(
                        "api",
                        NodeKind::Directory,
                        vec![node("a.rs", NodeKind::File, vec![])],
                    ),
                    node(
                        "generated",
                        NodeKind::Directory,
                        vec![node("b.rs", NodeKind::File, vec![])],
                    ),
                    node(
                        "api_docs",
                        NodeKind::Directory,
                        vec![node("visible.rs", NodeKind::File, vec![])],
                    ),
                ],
            ),
        };
        let options = AsciiOptions {
            rules: VisibilityRules {
                rules: vec![
                    PatternRule {
                        pattern: "api".into(),
                        visibility: Visibility::Collapse,
                    },
                    PatternRule {
                        pattern: "generated".into(),
                        visibility: Visibility::Collapse,
                    },
                ],
            },
            ..AsciiOptions::default()
        };
        let output = render_ascii(&tree, &options);

        assert!(output.contains("api/ [...]"));
        assert!(output.contains("generated/ [...]"));
        assert!(output.contains("api_docs/"));
        assert!(output.contains("visible.rs"));
    }

    #[test]
    fn repeated_omit_rules_hide_directories_and_descendants() {
        let tree = FileTree {
            root: node(
                "project",
                NodeKind::Directory,
                vec![
                    node(
                        "api",
                        NodeKind::Directory,
                        vec![node("secret.rs", NodeKind::File, vec![])],
                    ),
                    node("generated", NodeKind::Directory, vec![]),
                    node("src", NodeKind::Directory, vec![]),
                ],
            ),
        };
        let options = AsciiOptions {
            rules: VisibilityRules {
                rules: vec![
                    PatternRule {
                        pattern: "api".into(),
                        visibility: Visibility::Omit,
                    },
                    PatternRule {
                        pattern: "generated".into(),
                        visibility: Visibility::Omit,
                    },
                ],
            },
            ..AsciiOptions::default()
        };
        let output = render_ascii(&tree, &options);

        assert!(!output.contains("api/"));
        assert!(!output.contains("secret.rs"));
        assert!(!output.contains("generated/"));
        assert!(output.contains("src/"));
    }

    #[test]
    fn wildcard_and_exact_rules_never_apply_to_files() {
        let tree = FileTree {
            root: node(
                "project",
                NodeKind::Directory,
                vec![
                    node(
                        "foo.egg-info",
                        NodeKind::Directory,
                        vec![node("PKG-INFO", NodeKind::File, vec![])],
                    ),
                    node("bar.egg-info", NodeKind::File, vec![]),
                    node(".gitignore", NodeKind::File, vec![]),
                    node("object_info.rst", NodeKind::File, vec![]),
                    node("cachefile.rst", NodeKind::File, vec![]),
                ],
            ),
        };
        let options = AsciiOptions {
            rules: VisibilityRules {
                rules: ["*.egg-info", ".git", "obj", "cache"]
                    .into_iter()
                    .map(|pattern| PatternRule {
                        pattern: pattern.into(),
                        visibility: Visibility::Collapse,
                    })
                    .collect(),
            },
            ..AsciiOptions::default()
        };
        let output = render_ascii(&tree, &options);

        assert!(output.contains("foo.egg-info/ [...]"));
        assert!(output.contains("bar.egg-info"));
        assert!(!output.contains("bar.egg-info [...]"));
        assert!(output.contains(".gitignore"));
        assert!(output.contains("object_info.rst"));
        assert!(output.contains("cachefile.rst"));
        assert!(!output.contains(".gitignore [...]"));
        assert!(!output.contains("object_info.rst [...]"));
        assert!(!output.contains("cachefile.rst [...]"));
    }
}
