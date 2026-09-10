//! Lossless metadata editing for PNG and WebP containers.
use std::{fs, path::Path};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported image container (expected PNG or WebP)")]
    Unsupported,
    #[error("invalid {0} container")]
    Invalid(&'static str),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
    Png,
    WebP,
}
pub fn format(b: &[u8]) -> Result<Format, Error> {
    if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        Ok(Format::Png)
    } else if b.starts_with(b"RIFF") && b.get(8..12) == Some(b"WEBP") {
        Ok(Format::WebP)
    } else {
        Err(Error::Unsupported)
    }
}

pub fn webp_exif(b: &[u8]) -> Result<Option<&[u8]>, Error> {
    if format(b)? != Format::WebP {
        return Err(Error::Unsupported);
    }
    let mut p = 12;
    while p + 8 <= b.len() {
        let n = u32::from_le_bytes(b[p + 4..p + 8].try_into().unwrap()) as usize;
        let end = p.checked_add(8 + n).ok_or(Error::Invalid("WebP"))?;
        if end > b.len() {
            return Err(Error::Invalid("WebP"));
        }
        if &b[p..p + 4] == b"EXIF" {
            return Ok(Some(&b[p + 8..end]));
        }
        p = end + (n & 1)
    }
    Ok(None)
}

pub fn replace_webp_exif(b: &[u8], exif: &[u8]) -> Result<Vec<u8>, Error> {
    if format(b)? != Format::WebP {
        return Err(Error::Unsupported);
    }
    let mut chunks: Vec<([u8; 4], Vec<u8>)> = Vec::new();
    let mut p = 12;
    while p + 8 <= b.len() {
        let kind = b[p..p + 4].try_into().unwrap();
        let n = u32::from_le_bytes(b[p + 4..p + 8].try_into().unwrap()) as usize;
        let end = p.checked_add(8 + n).ok_or(Error::Invalid("WebP"))?;
        if end > b.len() {
            return Err(Error::Invalid("WebP"));
        }
        if &kind != b"EXIF" {
            chunks.push((kind, b[p + 8..end].to_vec()))
        }
        p = end + (n & 1)
    }
    if p != b.len() {
        return Err(Error::Invalid("WebP"));
    }
    if let Some((_, d)) = chunks.iter_mut().find(|(k, _)| k == b"VP8X") {
        if d.len() < 10 {
            return Err(Error::Invalid("WebP"));
        }
        d[0] |= 0x08
    } else {
        let (w, h) = webp_dimensions(&chunks).ok_or(Error::Invalid("WebP"))?;
        let mut d = vec![0x08, 0, 0, 0];
        d.extend_from_slice(&(w - 1).to_le_bytes()[..3]);
        d.extend_from_slice(&(h - 1).to_le_bytes()[..3]);
        chunks.insert(0, (*b"VP8X", d))
    }
    chunks.push((*b"EXIF", exif.to_vec()));
    let mut out = b"RIFF\0\0\0\0WEBP".to_vec();
    for (k, d) in chunks {
        out.extend_from_slice(&k);
        out.extend_from_slice(&(d.len() as u32).to_le_bytes());
        out.extend_from_slice(&d);
        if d.len() & 1 != 0 {
            out.push(0)
        }
    }
    let size = u32::try_from(out.len() - 8).map_err(|_| Error::Invalid("WebP"))?;
    out[4..8].copy_from_slice(&size.to_le_bytes());
    Ok(out)
}
fn webp_dimensions(c: &[([u8; 4], Vec<u8>)]) -> Option<(u32, u32)> {
    for (k, d) in c {
        if k == b"VP8 " && d.len() >= 10 && d[3..6] == [0x9d, 1, 0x2a] {
            return Some((
                (u16::from_le_bytes([d[6], d[7]]) & 0x3fff) as u32,
                (u16::from_le_bytes([d[8], d[9]]) & 0x3fff) as u32,
            ));
        }
        if k == b"VP8L" && d.len() >= 5 && d[0] == 0x2f {
            let x = u32::from_le_bytes([d[1], d[2], d[3], d[4]]);
            return Some(((x & 0x3fff) + 1, ((x >> 14) & 0x3fff) + 1));
        }
    }
    None
}

pub fn replace_png_text(b: &[u8], r: &[(&str, &str)]) -> Result<Vec<u8>, Error> {
    if format(b)? != Format::Png {
        return Err(Error::Unsupported);
    }
    let mut out = b[..8].to_vec();
    let mut p = 8;
    let mut inserted = false;
    while p + 12 <= b.len() {
        let n = u32::from_be_bytes(b[p..p + 4].try_into().unwrap()) as usize;
        let end = p.checked_add(12 + n).ok_or(Error::Invalid("PNG"))?;
        if end > b.len() {
            return Err(Error::Invalid("PNG"));
        }
        let kind = &b[p + 4..p + 8];
        let data = &b[p + 8..p + 8 + n];
        let replaced = matches!(kind, b"tEXt" | b"iTXt" | b"zTXt")
            && data
                .split(|x| *x == 0)
                .next()
                .and_then(|x| std::str::from_utf8(x).ok())
                .is_some_and(|key| r.iter().any(|(wanted, _)| key == *wanted));
        if !inserted && matches!(kind, b"IDAT" | b"IEND") {
            for (key, value) in r {
                append_itxt(&mut out, key, value)
            }
            inserted = true;
        }
        if !replaced {
            out.extend_from_slice(&b[p..end])
        }
        p = end;
        if kind == b"IEND" {
            break;
        }
    }
    if p != b.len() {
        return Err(Error::Invalid("PNG"));
    }
    Ok(out)
}
fn append_itxt(out: &mut Vec<u8>, key: &str, value: &str) {
    let mut d = Vec::new();
    d.extend_from_slice(key.as_bytes());
    d.extend_from_slice(b"\0\0\0\0\0");
    d.extend_from_slice(value.as_bytes());
    out.extend_from_slice(&(d.len() as u32).to_be_bytes());
    out.extend_from_slice(b"iTXt");
    out.extend_from_slice(&d);
    let mut c = crc32fast::Hasher::new();
    c.update(b"iTXt");
    c.update(&d);
    out.extend_from_slice(&c.finalize().to_be_bytes())
}
pub fn write_atomic(path: &Path, b: &[u8]) -> Result<(), Error> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temp = tempfile::Builder::new()
        .prefix(".image-meta-")
        .suffix(".tmp")
        .tempfile_in(parent)?;
    let candidate = temp.into_temp_path();
    fs::write(&candidate, b)?;
    candidate.persist(path).map_err(|e| Error::Io(e.error))?;
    Ok(())
}

pub fn exif_fields(exif: &[u8]) -> Option<(Option<&[u8]>, Option<&[u8]>)> {
    let t = exif.strip_prefix(b"Exif\0\0").unwrap_or(exif);
    let le = match t.get(..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let u16at = |p: usize| {
        let x = t.get(p..p + 2)?;
        Some(if le {
            u16::from_le_bytes([x[0], x[1]])
        } else {
            u16::from_be_bytes([x[0], x[1]])
        })
    };
    let u32at = |p: usize| {
        let x = t.get(p..p + 4)?;
        Some(if le {
            u32::from_le_bytes(x.try_into().ok()?)
        } else {
            u32::from_be_bytes(x.try_into().ok()?)
        })
    };
    let ifd = u32at(4)? as usize;
    let count = u16at(ifd)? as usize;
    let (mut desc, mut ep) = (None, None);
    for i in 0..count {
        let p = ifd + 2 + i * 12;
        let tag = u16at(p)?;
        let n = u32at(p + 4)? as usize;
        let off = u32at(p + 8)? as usize;
        if tag == 0x010e {
            desc = t.get(off..off + n)
        } else if tag == 0x8769 {
            ep = Some(off)
        }
    }
    let mut comment = None;
    if let Some(p) = ep {
        for i in 0..u16at(p)? as usize {
            let e = p + 2 + i * 12;
            if u16at(e)? == 0x9286 {
                let n = u32at(e + 4)? as usize;
                let off = u32at(e + 8)? as usize;
                comment = t.get(off..off + n)
            }
        }
    }
    Some((desc, comment))
}
pub fn build_exif(description: Option<&[u8]>, comment: Option<&[u8]>) -> Vec<u8> {
    let n = if description.is_some() { 2 } else { 1 };
    let ifd = 8;
    let ep = ifd + 2 + n * 12 + 4;
    let start = ep + 2 + comment.is_some() as usize * 12 + 4;
    let mut t = vec![0; start];
    t[..2].copy_from_slice(b"MM");
    t[2..4].copy_from_slice(&42u16.to_be_bytes());
    t[4..8].copy_from_slice(&8u32.to_be_bytes());
    t[8..10].copy_from_slice(&(n as u16).to_be_bytes());
    let mut e = 10;
    let mut extra = Vec::new();
    if let Some(d) = description {
        let mut v = d.to_vec();
        v.push(0);
        entry(
            &mut t,
            e,
            0x010e,
            2,
            v.len() as u32,
            (start + extra.len()) as u32,
        );
        extra.extend(v);
        if extra.len() & 1 != 0 {
            extra.push(0)
        }
        e += 12
    }
    entry(&mut t, e, 0x8769, 4, 1, ep as u32);
    t[ep..ep + 2].copy_from_slice(&(comment.is_some() as u16).to_be_bytes());
    if let Some(c) = comment {
        entry(
            &mut t,
            ep + 2,
            0x9286,
            7,
            c.len() as u32,
            (start + extra.len()) as u32,
        );
        extra.extend_from_slice(c)
    }
    t.extend(extra);
    t
}
fn entry(t: &mut [u8], p: usize, tag: u16, kind: u16, count: u32, value: u32) {
    t[p..p + 2].copy_from_slice(&tag.to_be_bytes());
    t[p + 2..p + 4].copy_from_slice(&kind.to_be_bytes());
    t[p + 4..p + 8].copy_from_slice(&count.to_be_bytes());
    t[p + 8..p + 12].copy_from_slice(&value.to_be_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exif_roundtrip() {
        let e = build_exif(Some(b"Workflow:{}"), Some(b"ASCII\0\0\0Prompt:{}"));
        let f = exif_fields(&e).unwrap();
        assert_eq!(f.0.unwrap().strip_suffix(&[0]).unwrap(), b"Workflow:{}");
        assert_eq!(f.1.unwrap(), b"ASCII\0\0\0Prompt:{}")
    }
}
