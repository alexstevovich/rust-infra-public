//! ComfyUI workflow and prompt metadata for PNG and WebP images.

use image_container_meta::{
    Format, build_exif, exif_fields, replace_png_text, replace_webp_exif, webp_exif,
};
use std::io::Cursor;
use thiserror::Error;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Metadata {
    pub prompt: Option<serde_json::Value>,
    pub workflow: serde_json::Value,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("container metadata error: {0}")]
    Container(#[from] image_container_meta::Error),
    #[error("invalid PNG metadata: {0}")]
    Png(#[from] png::DecodingError),
    #[error("invalid ComfyUI JSON in '{key}': {source}")]
    Json {
        key: &'static str,
        source: serde_json::Error,
    },
    #[error("ComfyUI prompt exists but workflow is missing")]
    MissingWorkflow,
    #[error("embedded ComfyUI metadata could not be verified")]
    Verification,
}

/// Extracts ComfyUI workflow and optional prompt metadata from PNG or WebP bytes.
pub fn extract(bytes: &[u8]) -> Result<Option<Metadata>, Error> {
    match image_container_meta::format(bytes)? {
        Format::Png => extract_png(bytes),
        Format::WebP => extract_webp(bytes),
    }
}

/// Embeds ComfyUI metadata into PNG or WebP bytes without re-encoding pixels.
pub fn embed(bytes: &[u8], metadata: &Metadata) -> Result<Vec<u8>, Error> {
    let updated = match image_container_meta::format(bytes)? {
        Format::Png => {
            let workflow = to_json("workflow", &metadata.workflow)?;
            let prompt = metadata
                .prompt
                .as_ref()
                .map(|value| to_json("prompt", value))
                .transpose()?;
            let mut fields = vec![("workflow", workflow.as_str())];
            if let Some(prompt) = prompt.as_deref() {
                fields.push(("prompt", prompt));
            }
            replace_png_text(bytes, &fields)?
        }
        Format::WebP => {
            let workflow = format!("Workflow:{}", to_json("workflow", &metadata.workflow)?);
            let prompt = metadata
                .prompt
                .as_ref()
                .map(|value| {
                    let mut encoded = b"ASCII\0\0\0Prompt:".to_vec();
                    encoded.extend_from_slice(to_json("prompt", value)?.as_bytes());
                    Ok::<_, Error>(encoded)
                })
                .transpose()?;
            let exif = build_exif(Some(workflow.as_bytes()), prompt.as_deref());
            replace_webp_exif(bytes, &exif)?
        }
    };

    if extract(&updated)?.as_ref() != Some(metadata) {
        return Err(Error::Verification);
    }
    Ok(updated)
}

fn to_json(key: &'static str, value: &serde_json::Value) -> Result<String, Error> {
    serde_json::to_string(value).map_err(|source| Error::Json { key, source })
}

fn extract_png(bytes: &[u8]) -> Result<Option<Metadata>, Error> {
    let reader = png::Decoder::new(Cursor::new(bytes)).read_info()?;
    let info = reader.info();
    let get = |key: &str| {
        info.uncompressed_latin1_text
            .iter()
            .find(|entry| entry.keyword == key)
            .map(|entry| entry.text.clone())
            .or_else(|| {
                info.utf8_text
                    .iter()
                    .find(|entry| entry.keyword == key)
                    .and_then(|entry| entry.get_text().ok())
            })
    };
    let prompt = parse_json("prompt", get("prompt"))?;
    let workflow = parse_json("workflow", get("workflow"))?;
    match workflow {
        Some(workflow) => Ok(Some(Metadata { prompt, workflow })),
        None if prompt.is_some() => Err(Error::MissingWorkflow),
        None => Ok(None),
    }
}

fn extract_webp(bytes: &[u8]) -> Result<Option<Metadata>, Error> {
    let Some(exif) = webp_exif(bytes)? else {
        return Ok(None);
    };
    let Some((description, comment)) = exif_fields(exif) else {
        return Ok(None);
    };
    let workflow = description
        .map(|value| value.strip_suffix(&[0]).unwrap_or(value))
        .and_then(|value| std::str::from_utf8(value).ok())
        .and_then(|value| value.strip_prefix("Workflow:"))
        .map(str::to_owned);
    let prompt = comment
        .and_then(|value| value.strip_prefix(b"ASCII\0\0\0Prompt:"))
        .and_then(|value| std::str::from_utf8(value).ok())
        .map(str::to_owned);
    let prompt = parse_json("prompt", prompt)?;
    Ok(parse_json("workflow", workflow)?.map(|workflow| Metadata { prompt, workflow }))
}

fn parse_json(
    key: &'static str,
    value: Option<String>,
) -> Result<Option<serde_json::Value>, Error> {
    value
        .map(|value| serde_json::from_str(&value).map_err(|source| Error::Json { key, source }))
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::{Metadata, embed, extract};
    use std::io::Cursor;

    fn webp_pixels(bytes: &[u8]) -> &[u8] {
        let mut position = 12;
        while position + 8 <= bytes.len() {
            let length =
                u32::from_le_bytes(bytes[position + 4..position + 8].try_into().unwrap()) as usize;
            if matches!(&bytes[position..position + 4], b"VP8 " | b"VP8L" | b"ANMF") {
                return &bytes[position + 8..position + 8 + length];
            }
            position += 8 + length + (length & 1);
        }
        panic!("WebP pixel chunk missing")
    }

    #[test]
    fn png_round_trip_preserves_metadata() {
        let mut image = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut image, 1, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[1, 2, 3, 255]).unwrap();
        }
        let metadata = Metadata {
            prompt: Some(serde_json::json!({"1": {"class_type": "KSampler"}})),
            workflow: serde_json::json!({"nodes": [{"type": "LoraInfo"}]}),
        };
        let updated = embed(&image, &metadata).unwrap();
        assert_eq!(extract(&updated).unwrap(), Some(metadata));
    }

    #[test]
    fn webp_round_trip_preserves_pixels_and_lora_nodes() {
        let mut image = Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(2, 2)
            .write_to(&mut image, image::ImageFormat::WebP)
            .unwrap();
        let image = image.into_inner();
        let metadata = Metadata {
            prompt: Some(serde_json::json!({"1": {"class_type": "KSampler"}})),
            workflow: serde_json::json!({"nodes": [{"type": "LoraInfo"}]}),
        };
        let updated = embed(&image, &metadata).unwrap();
        assert_eq!(webp_pixels(&updated), webp_pixels(&image));
        assert_eq!(extract(&updated).unwrap(), Some(metadata));
    }
}
