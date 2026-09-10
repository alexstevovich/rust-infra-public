# filetree

`filetree` is a small Rust command-line tool that turns a filesystem hierarchy
into a clean, shareable tree. Its primary output is ANSI-colored ASCII intended
for AI context, documentation, guides, issue reports, and humans inspecting a
project's structure. Plain JSON is also available for tooling and integrations.

## Architecture

```text
filetree-cli/
├── ../filetree-maker/       filesystem and NTFS/MFT hierarchy builder
├── ../filetree-visualizer/  ASCII rendering and display filtering
└── src/                      executable and composition root
```

`filetree-maker` owns the canonical hierarchical `FileTree` and `FileNode` model.
Each node records its name, path, kind, optional size, symlink/reparse state,
scan status, and child nodes. It does not contain presentation rules.

`filetree-visualizer` consumes a `FileTree`, applies visibility rules, and renders
ASCII or serializes JSON. ANSI styling exists only in the ASCII renderer; the
underlying tree remains presentation-neutral and JSON contains no styling.

`filetree` parses command-line options and composes the other two crates.

## Scanning

The default scanner performs portable recursive filesystem traversal. On
Windows, `--mft` uses `FSCTL_ENUM_USN_DATA` to enumerate an NTFS volume's MFT
records and reconstruct the requested subtree quickly. Because that API does
not provide logical file sizes, `--mft` cannot be combined with `--sizes` or
`--follow-symlinks`.

MFT scanning starts a short-lived copy of `filetree` through Windows UAC. The
elevated helper builds the tree, returns it through a temporary JSON file, and
exits. The normal process waits for the result and removes the temporary file.
Ordinary scans do not request elevation.

## Output and filtering

ASCII output uses bold true-color styling for tree flow, normal nodes, and
collapsed directories. JSON output is selected with `--format json` and always
remains plain.

`--collapse` shows a matching directory but hides its descendants as
`directory/ [...]`. `--omit` hides both a matching directory and its
descendants. Both options are repeatable, and omit wins when both match.

Rules currently match a single **directory name** anywhere in the hierarchy;
they never match files or full paths. An exact rule such as `--collapse api`
matches only directories named `api`. A rule may contain `*` as a minimal
directory-name wildcard, so `--collapse "*.egg-info"` matches directories such
as `package.egg-info`.

`--preset common` contributes conservative collapse rules for familiar noisy,
generated, cache, and build directories such as `.git`, `__pycache__`,
`node_modules`, `target`, and `*.egg-info`. It collapses these directories; it
does not remove them. Explicit collapse rules are combined with the preset.

## Examples

```powershell
cargo run -p alexstevovich-filetree-cli -- .
cargo run -p alexstevovich-filetree-cli -- . --preset common
cargo run -p alexstevovich-filetree-cli -- . --collapse api --collapse generated
cargo run -p alexstevovich-filetree-cli -- . --omit api
cargo run -p alexstevovich-filetree-cli -- . --collapse "*.egg-info"

cargo run -p alexstevovich-filetree-cli -- "C:\delete" --mft --preset common
cargo run -p alexstevovich-filetree-cli -- "C:\delete" --mft --preset common --collapse api --collapse manual
cargo run -p alexstevovich-filetree-cli -- "C:\delete" --mft --collapse "*.egg-info"
```

Other useful options include `--max-depth`, `--sizes`, and `--format json`. Run
`filetree --help` for the complete command reference.

## Release build

Build the optimized executable with:

```powershell
cargo build --release -p alexstevovich-filetree-cli
```

The resulting executable is `target\release\filetree.exe`. The root `target/`
directory contains generated Cargo build artifacts and is intentionally
excluded from Git; `Cargo.lock` is committed because this workspace produces an
application.
