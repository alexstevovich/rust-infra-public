use clap::{Parser, ValueEnum};
use filetree_maker::{
    ScanOptions, is_elevated_scan_command, request_elevated_scan, run_elevated_scan, scan,
};
use filetree_visualizer::{
    AsciiOptions, PatternRule, Visibility, VisibilityRules, render_ascii, render_json,
};
use std::collections::HashSet;
use std::path::PathBuf;

const COMMON_COLLAPSE_DIRECTORIES: &[&str] = &[
    ".git",
    ".svn",
    ".hg",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    ".tox",
    ".nox",
    ".venv",
    "venv",
    "node_modules",
    ".npm",
    ".yarn",
    ".pnpm-store",
    "target",
    "bin",
    "obj",
    ".gradle",
    ".next",
    ".nuxt",
    ".svelte-kit",
    "dist",
    "build",
    "coverage",
    "htmlcov",
    ".cache",
    "DerivedDataCache",
    "Intermediate",
    "Saved",
    "Binaries",
    "*.egg-info",
];

#[derive(Debug, Parser)]
#[command(
    name = "filetree",
    version,
    about = "Shareable filesystem tree output for humans and AI"
)]
struct Cli {
    /// Root path to scan.
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Ascii)]
    format: OutputFormat,

    /// Show file sizes when available.
    #[arg(long)]
    sizes: bool,

    /// Use elevated NTFS MFT enumeration (Windows only; file sizes are unavailable).
    #[arg(long, conflicts_with_all = ["sizes", "follow_symlinks"])]
    mft: bool,

    /// Follow filesystem symlinks while scanning.
    #[arg(long)]
    follow_symlinks: bool,

    /// Maximum traversal depth. Root is depth 0.
    #[arg(long)]
    max_depth: Option<usize>,

    /// Apply a built-in set of useful display rules.
    #[arg(long, value_enum)]
    preset: Option<Preset>,

    /// Show the node but hide its descendants. Repeatable.
    #[arg(long = "collapse")]
    collapse: Vec<String>,

    /// Remove matching nodes from rendered output. Repeatable.
    #[arg(long = "omit")]
    omit: Vec<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Ascii,
    Json,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Preset {
    Common,
}

fn main() {
    let raw_args: Vec<String> = std::env::args().collect();

    if is_elevated_scan_command(&raw_args) {
        if let Err(error) = run_elevated_scan(&raw_args) {
            eprintln!("filetree elevated scan failed: {error}");
            std::process::exit(1);
        }
        return;
    }

    let cli = Cli::parse();
    let options = ScanOptions {
        follow_symlinks: cli.follow_symlinks,
        max_depth: cli.max_depth,
    };

    let tree_result = if cli.mft {
        request_elevated_scan(&cli.path).map_err(|error| error.to_string())
    } else {
        scan(&cli.path, &options).map_err(|error| error.to_string())
    };
    let mut tree = match tree_result {
        Ok(tree) => tree,
        Err(error) => {
            eprintln!("filetree: {error}");
            std::process::exit(1);
        }
    };
    if cli.mft
        && let Some(max_depth) = cli.max_depth
    {
        tree.truncate_to_depth(max_depth);
    }

    let rules = build_visibility_rules(cli.preset, cli.collapse, cli.omit);

    match cli.format {
        OutputFormat::Ascii => {
            let options = AsciiOptions {
                show_sizes: cli.sizes,
                mark_symlinks: true,
                rules,
            };
            print!("{}", render_ascii(&tree, &options));
        }
        OutputFormat::Json => match render_json(&tree) {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("filetree: could not serialize JSON: {error}");
                std::process::exit(1);
            }
        },
    }
}

fn build_visibility_rules(
    preset: Option<Preset>,
    explicit_collapse: Vec<String>,
    omit: Vec<String>,
) -> VisibilityRules {
    let mut seen_omit = HashSet::new();
    let omit: Vec<String> = omit
        .into_iter()
        .filter(|pattern| seen_omit.insert(pattern.clone()))
        .collect();
    let omitted: HashSet<&str> = omit.iter().map(String::as_str).collect();
    let preset_patterns = match preset {
        Some(Preset::Common) => COMMON_COLLAPSE_DIRECTORIES,
        None => &[],
    };
    let mut seen_collapse = HashSet::new();
    let collapse: Vec<String> = preset_patterns
        .iter()
        .map(|pattern| (*pattern).to_owned())
        .chain(explicit_collapse)
        .filter(|pattern| !omitted.contains(pattern.as_str()))
        .filter(|pattern| seen_collapse.insert(pattern.clone()))
        .collect();

    VisibilityRules {
        rules: collapse
            .into_iter()
            .map(|pattern| PatternRule {
                pattern,
                visibility: Visibility::Collapse,
            })
            .chain(omit.into_iter().map(|pattern| PatternRule {
                pattern,
                visibility: Visibility::Omit,
            }))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_preset_combines_with_explicit_collapse_without_duplicates() {
        let rules = build_visibility_rules(
            Some(Preset::Common),
            vec!["target".into(), "SomeOtherFolder".into()],
            vec![],
        );
        let patterns: HashSet<&str> = rules
            .rules
            .iter()
            .map(|rule| rule.pattern.as_str())
            .collect();

        assert_eq!(patterns.len(), rules.rules.len());
        assert!(patterns.contains(".git"));
        assert!(patterns.contains("target"));
        assert!(patterns.contains("*.egg-info"));
        assert!(patterns.contains("SomeOtherFolder"));
        assert!(!patterns.contains(".vscode"));
        assert!(
            rules
                .rules
                .iter()
                .all(|rule| rule.visibility == Visibility::Collapse)
        );
    }
}
