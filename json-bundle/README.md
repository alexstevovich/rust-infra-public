# json-bundle

Reusable Rust library for reversibly bundling JSON object files into an editable array. Each object records its original absolute source path so it can later be restored.

The primary operations are `pack` and `unpack`. Bundles are intended for local workflows because their bookkeeping contains absolute filesystem paths.
