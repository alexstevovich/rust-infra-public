# filetree-maker

Reusable Rust library that scans filesystem hierarchies into a structured `FileTree` model.

The default scanner uses portable recursive traversal. On Windows, the library also supports elevated NTFS MFT enumeration for fast hierarchy construction. It owns scanning and the presentation-neutral tree model; rendering belongs to `filetree-visualizer`.
