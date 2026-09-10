# json-bundle

`json-bundle` reversibly combines independent JSON object files into one editable JSON array. Each bundled object receives its original absolute path, allowing `unpack` to write it back to the exact source location.

```powershell
json-bundle pack "writing/*/article.json" output.json
json-bundle unpack output.json
```

Before:

```json
{
  "title": "Foo",
  "body": "Hello"
}
```

Bundled:

```json
[
  {
    "__source_path": "E:\\my\\writing\\foo\\article.json",
    "title": "Foo",
    "body": "Hello"
  }
]
```

After unpacking, the original file again contains:

```json
{
  "title": "Foo",
  "body": "Hello"
}
```

`__source_path` is temporary bookkeeping in the bundle and is removed from each restored object. Use `--path-key <key>` with either subcommand to select a different reserved property.

Bundles intentionally contain absolute filesystem paths. They are primarily local working or interchange files, not portable archives.
