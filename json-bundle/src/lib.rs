//! Reversible JSON object bundling, independent of CLI argument parsing.

use glob::{MatchOptions, glob_with};
use serde_json::{Map, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use tempfile::{NamedTempFile, TempPath};
use thiserror::Error;

pub const DEFAULT_PATH_KEY: &str = "__source_path";

#[derive(Debug, Error)]
pub enum Error {
    #[error("no files matched the supplied inputs")]
    NoMatches,
    #[error("invalid glob pattern '{pattern}': {source}")]
    GlobPattern {
        pattern: String,
        source: glob::PatternError,
    },
    #[error("could not evaluate glob '{pattern}': {source}")]
    GlobWalk {
        pattern: String,
        source: glob::GlobError,
    },
    #[error("could not canonicalize '{path}': {source}")]
    Canonicalize {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not read '{path}': {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("malformed JSON in '{path}': {source}")]
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("JSON in '{0}' must have an object at the top level")]
    SourceNotObject(PathBuf),
    #[error("JSON object in '{path}' already contains reserved path key '{key}'")]
    ReservedKey { path: PathBuf, key: String },
    #[error("absolute path '{0}' cannot be represented as a JSON string")]
    NonUnicodePath(PathBuf),
    #[error("bundle '{0}' must contain an array at the top level")]
    BundleNotArray(PathBuf),
    #[error("bundle element {index} must be an object")]
    ElementNotObject { index: usize },
    #[error("bundle element {index} is missing path key '{key}'")]
    MissingPathKey { index: usize, key: String },
    #[error("path key '{key}' in bundle element {index} must be a string")]
    PathNotString { index: usize, key: String },
    #[error("path '{path}' in bundle element {index} is not absolute")]
    PathNotAbsolute { index: usize, path: PathBuf },
    #[error("parent directory does not exist for '{0}'")]
    MissingParent(PathBuf),
    #[error("could not serialize JSON for '{path}': {source}")]
    Serialize {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("could not create a temporary file beside '{path}': {source}")]
    CreateTemp {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not write temporary output for '{path}': {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not atomically replace '{path}': {source}")]
    Commit {
        path: PathBuf,
        source: std::io::Error,
    },
}

pub fn pack(inputs: &[String], output: &Path, path_key: &str) -> Result<usize, Error> {
    let paths = expand_inputs(inputs)?;
    let mut objects = Vec::with_capacity(paths.len());
    for path in &paths {
        let bytes = fs::read(path).map_err(|source| Error::Read {
            path: path.clone(),
            source,
        })?;
        let value: Value = serde_json::from_slice(&bytes).map_err(|source| Error::Json {
            path: path.clone(),
            source,
        })?;
        let mut object = value
            .as_object()
            .cloned()
            .ok_or_else(|| Error::SourceNotObject(path.clone()))?;
        if object.contains_key(path_key) {
            return Err(Error::ReservedKey {
                path: path.clone(),
                key: path_key.into(),
            });
        }
        let absolute = portable_absolute(path)?;
        object.insert(path_key.into(), Value::String(absolute.into()));
        objects.push(Value::Object(object));
    }
    let bytes =
        serde_json::to_vec_pretty(&Value::Array(objects)).map_err(|source| Error::Serialize {
            path: output.into(),
            source,
        })?;
    transactional_write(output, &bytes)?;
    Ok(paths.len())
}

fn portable_absolute(path: &Path) -> Result<String, Error> {
    let text = path
        .to_str()
        .ok_or_else(|| Error::NonUnicodePath(path.into()))?;
    #[cfg(windows)]
    {
        if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
            return Ok(format!(r"\\{rest}"));
        }
        if let Some(rest) = text.strip_prefix(r"\\?\") {
            return Ok(rest.into());
        }
    }
    Ok(text.into())
}

pub fn unpack(bundle: &Path, path_key: &str) -> Result<usize, Error> {
    let bytes = fs::read(bundle).map_err(|source| Error::Read {
        path: bundle.into(),
        source,
    })?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|source| Error::Json {
        path: bundle.into(),
        source,
    })?;
    let array = value
        .as_array()
        .ok_or_else(|| Error::BundleNotArray(bundle.into()))?;

    // Validate and serialize every entry before touching any destination.
    let mut writes = Vec::with_capacity(array.len());
    for (index, value) in array.iter().enumerate() {
        let mut object: Map<String, Value> = value
            .as_object()
            .cloned()
            .ok_or(Error::ElementNotObject { index })?;
        let path_value = object
            .remove(path_key)
            .ok_or_else(|| Error::MissingPathKey {
                index,
                key: path_key.into(),
            })?;
        let path_text = path_value.as_str().ok_or_else(|| Error::PathNotString {
            index,
            key: path_key.into(),
        })?;
        let path = PathBuf::from(path_text);
        if !path.is_absolute() {
            return Err(Error::PathNotAbsolute { index, path });
        }
        ensure_parent(&path)?;
        let bytes = serde_json::to_vec_pretty(&Value::Object(object)).map_err(|source| {
            Error::Serialize {
                path: path.clone(),
                source,
            }
        })?;
        writes.push((path, bytes));
    }

    // Prepare every complete temporary file before replacing any destination.
    let mut prepared = Vec::with_capacity(writes.len());
    for (path, bytes) in &writes {
        prepared.push((path.clone(), prepare(path, bytes)?));
    }
    for (path, temporary) in prepared {
        commit(temporary, &path)?;
    }
    Ok(writes.len())
}

fn expand_inputs(inputs: &[String]) -> Result<Vec<PathBuf>, Error> {
    let mut unique = BTreeMap::<PathBuf, PathBuf>::new();
    for input in inputs {
        let matches = glob_with(
            input,
            MatchOptions {
                case_sensitive: !cfg!(windows),
                ..Default::default()
            },
        )
        .map_err(|source| Error::GlobPattern {
            pattern: input.clone(),
            source,
        })?;
        for entry in matches {
            let path = entry.map_err(|source| Error::GlobWalk {
                pattern: input.clone(),
                source,
            })?;
            if !path.is_file() {
                continue;
            }
            let canonical = fs::canonicalize(&path).map_err(|source| Error::Canonicalize {
                path: path.clone(),
                source,
            })?;
            let key = if cfg!(windows) {
                PathBuf::from(canonical.to_string_lossy().to_lowercase())
            } else {
                canonical.clone()
            };
            unique.entry(key).or_insert(canonical);
        }
    }
    if unique.is_empty() {
        return Err(Error::NoMatches);
    }
    let mut paths: Vec<_> = unique.into_values().collect();
    paths.sort_by(|a, b| {
        if cfg!(windows) {
            a.to_string_lossy()
                .to_lowercase()
                .cmp(&b.to_string_lossy().to_lowercase())
        } else {
            a.cmp(b)
        }
    });
    Ok(paths)
}

fn ensure_parent(path: &Path) -> Result<&Path, Error> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| Error::MissingParent(path.into()))?;
    if !parent.is_dir() {
        return Err(Error::MissingParent(path.into()));
    }
    Ok(parent)
}
fn prepare(path: &Path, bytes: &[u8]) -> Result<TempPath, Error> {
    let parent = ensure_parent(path)?;
    let file = NamedTempFile::new_in(parent).map_err(|source| Error::CreateTemp {
        path: path.into(),
        source,
    })?;
    fs::write(file.path(), bytes).map_err(|source| Error::Write {
        path: path.into(),
        source,
    })?;
    Ok(file.into_temp_path())
}
fn commit(temporary: TempPath, path: &Path) -> Result<(), Error> {
    temporary
        .persist(path)
        .map(|_| ())
        .map_err(|error| Error::Commit {
            path: path.into(),
            source: error.error,
        })
}
fn transactional_write(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    commit(prepare(path, bytes)?, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn write(path: &Path, value: &Value) {
        fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
    }
    fn bundle(path: &Path) -> Value {
        serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
    }

    #[test]
    fn pack_multiple_absolute_deduplicated_and_deterministic() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("a.json");
        let b = d.path().join("b.json");
        let out = d.path().join("bundle.json");
        write(&b, &json!({"n":2}));
        write(&a, &json!({"n":1}));
        let inputs = vec![
            b.display().to_string(),
            format!("{}\\*.json", d.path().display()),
            a.display().to_string(),
        ];
        assert_eq!(pack(&inputs, &out, DEFAULT_PATH_KEY).unwrap(), 2);
        let v = bundle(&out);
        let rows = v.as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["n"], 1);
        for row in rows {
            assert!(Path::new(row[DEFAULT_PATH_KEY].as_str().unwrap()).is_absolute());
        }
    }

    #[test]
    fn pack_unpack_roundtrip_removes_bookkeeping_and_handles_spaces() {
        let d = tempfile::tempdir().unwrap();
        let folder = d.path().join("with spaces");
        fs::create_dir(&folder).unwrap();
        let source = folder.join("object file.json");
        let out = d.path().join("bundle.json");
        let original = json!({"title":"hello","nested":{"x":1}});
        write(&source, &original);
        pack(&[source.display().to_string()], &out, DEFAULT_PATH_KEY).unwrap();
        write(&source, &json!({"changed":true}));
        unpack(&out, DEFAULT_PATH_KEY).unwrap();
        let restored = bundle(&source);
        assert_eq!(restored, original);
        assert!(restored.get(DEFAULT_PATH_KEY).is_none());
    }

    #[test]
    fn reserved_key_and_non_object_are_rejected() {
        let d = tempfile::tempdir().unwrap();
        let source = d.path().join("x.json");
        let out = d.path().join("out.json");
        write(&source, &json!({DEFAULT_PATH_KEY:"bad"}));
        assert!(matches!(
            pack(&[source.display().to_string()], &out, DEFAULT_PATH_KEY),
            Err(Error::ReservedKey { .. })
        ));
        write(&source, &json!([1, 2]));
        assert!(matches!(
            pack(&[source.display().to_string()], &out, DEFAULT_PATH_KEY),
            Err(Error::SourceNotObject(_))
        ));
    }

    #[test]
    fn custom_path_key_roundtrips() {
        let d = tempfile::tempdir().unwrap();
        let source = d.path().join("x.json");
        let out = d.path().join("out.json");
        write(&source, &json!({"x":1}));
        pack(&[source.display().to_string()], &out, "where").unwrap();
        assert!(bundle(&out)[0].get("where").is_some());
        unpack(&out, "where").unwrap();
        assert_eq!(bundle(&source), json!({"x":1}));
    }

    #[test]
    fn malformed_bundles_are_rejected_before_existing_files_change() {
        let d = tempfile::tempdir().unwrap();
        let destination = d.path().join("keep.json");
        let out = d.path().join("bundle.json");
        write(&destination, &json!({"keep":true}));
        write(&out, &json!({"not":"array"}));
        assert!(matches!(
            unpack(&out, DEFAULT_PATH_KEY),
            Err(Error::BundleNotArray(_))
        ));
        write(
            &out,
            &json!([{DEFAULT_PATH_KEY:destination,"new":true},{"missing":"path"}]),
        );
        assert!(matches!(
            unpack(&out, DEFAULT_PATH_KEY),
            Err(Error::MissingPathKey { .. })
        ));
        assert_eq!(bundle(&destination), json!({"keep":true}));
        write(&out, &json!([{DEFAULT_PATH_KEY:42}]));
        assert!(matches!(
            unpack(&out, DEFAULT_PATH_KEY),
            Err(Error::PathNotString { .. })
        ));
        write(&out, &json!([{DEFAULT_PATH_KEY:"relative.json"}]));
        assert!(matches!(
            unpack(&out, DEFAULT_PATH_KEY),
            Err(Error::PathNotAbsolute { .. })
        ));
    }

    #[test]
    fn no_matches_fails() {
        let d = tempfile::tempdir().unwrap();
        let p = format!("{}\\missing-*.json", d.path().display());
        assert!(matches!(
            pack(&[p], &d.path().join("out.json"), DEFAULT_PATH_KEY),
            Err(Error::NoMatches)
        ));
    }

    #[cfg(windows)]
    #[test]
    fn windows_canonical_paths_keep_drive_absolute_form() {
        let d = tempfile::tempdir().unwrap();
        let s = d.path().join("x.json");
        let out = d.path().join("out.json");
        write(&s, &json!({}));
        pack(&[s.display().to_string()], &out, DEFAULT_PATH_KEY).unwrap();
        let p = bundle(&out)[0][DEFAULT_PATH_KEY]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(Path::new(&p).is_absolute());
        assert_eq!(p.as_bytes().get(1), Some(&b':'));
    }
}
