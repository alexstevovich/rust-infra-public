mod ascii;
mod rules;

pub use ascii::{AsciiOptions, render_ascii};
pub use rules::{PatternRule, Visibility, VisibilityRules};

use filetree_maker::FileTree;

pub fn render_json(tree: &FileTree) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(tree)
}
