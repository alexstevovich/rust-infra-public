# image-convert

`image-convert` is a low-level native conversion primitive: exactly one supported image input becomes exactly one supported image output. Directory discovery, batching, archive rules, size policy, source replacement, and deletion belong in external Python or `.cmd` wrappers. The source file is never mutated or deleted.

Output is transactional: encoding and image verification happen in a temporary file beside the destination. The verified candidate is then atomically persisted. A failed encode or verification leaves an existing destination untouched, including when `--overwrite` was requested.

Inputs: PNG, JPEG/JPG, WebP, BMP, TIFF, and GIF. Outputs: PNG, JPEG, WebP, and GIF. Still images and animated GIF/WebP are represented separately. In v0.1 animation is preserved (all frames, order, timing, and loop count); timing and frame editing are intentionally absent. Converting animation to a still-only output fails.

Still inputs produce ordinary non-animated WebP files; animated inputs produce WebP animation containers.

Spatial options are `--width`, `--height`, `--crop X Y WIDTH HEIGHT`, and `--rotate 0|90|180|270`; they apply to every animation frame. Explicit codec controls include `--jpeg-quality`, `--webp-quality`, `--webp-lossless`, `--webp-method`, and `--png-compression`. An option for the wrong output codec is an error.

```text
image-convert input.bmp output.png
image-convert input.png output.webp --webp-quality 90
image-convert animation.gif animation.webp
```

Build with `cargo build --release -p alexstevovich-image-convert-cli`. On Windows the executable is `target\release\image-convert.exe`.

## Crates

- `image-convert-cli`: argument parsing, path safety, transactional output, and user-facing errors
- `image-convert`: reusable codecs, still/animation model, transforms, and encoding
