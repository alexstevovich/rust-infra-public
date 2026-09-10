//! Automatic1111 and Forge generation metadata for PNG and WebP images.

use image_container_meta::{
    Format, build_exif, exif_fields, replace_png_text, replace_webp_exif, webp_exif,
};
use std::io::Cursor;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Metadata {
    pub parameters: String,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("container metadata error: {0}")]
    Container(#[from] image_container_meta::Error),
    #[error("invalid PNG metadata: {0}")]
    Png(#[from] png::DecodingError),
    #[error("embedded Automatic1111 metadata could not be verified")]
    Verification,
}

/// Extracts Automatic1111/Forge parameters metadata from PNG or WebP bytes.
pub fn extract(bytes: &[u8]) -> Result<Option<Metadata>, Error> {
    match image_container_meta::format(bytes)? {
        Format::Png => {
            let reader = png::Decoder::new(Cursor::new(bytes)).read_info()?;
            let info = reader.info();
            let value = info
                .uncompressed_latin1_text
                .iter()
                .find(|entry| entry.keyword == "parameters")
                .map(|entry| entry.text.clone())
                .or_else(|| {
                    info.utf8_text
                        .iter()
                        .find(|entry| entry.keyword == "parameters")
                        .and_then(|entry| entry.get_text().ok())
                });
            Ok(value.map(|parameters| Metadata { parameters }))
        }
        Format::WebP => {
            let Some(exif) = webp_exif(bytes)? else {
                return Ok(None);
            };
            let value = exif_fields(exif)
                .and_then(|(_, comment)| comment)
                .and_then(decode_webp_parameters);
            Ok(value.map(|parameters| Metadata { parameters }))
        }
    }
}

/// Embeds Automatic1111/Forge parameters into PNG or WebP bytes without re-encoding pixels.
pub fn embed(bytes: &[u8], metadata: &Metadata) -> Result<Vec<u8>, Error> {
    let updated = match image_container_meta::format(bytes)? {
        Format::Png => replace_png_text(bytes, &[("parameters", &metadata.parameters)])?,
        Format::WebP => {
            let mut comment = b"UNICODE\0".to_vec();
            for unit in metadata.parameters.encode_utf16() {
                comment.extend_from_slice(&unit.to_be_bytes());
            }
            let exif = build_exif(None, Some(&comment));
            replace_webp_exif(bytes, &exif)?
        }
    };
    if extract(&updated)?.as_ref() != Some(metadata) {
        return Err(Error::Verification);
    }
    Ok(updated)
}

fn decode_webp_parameters(value: &[u8]) -> Option<String> {
    let body = value.strip_prefix(b"UNICODE\0")?;
    if body.len() % 2 != 0 {
        return None;
    }
    String::from_utf16(
        &body
            .chunks_exact(2)
            .map(|bytes| u16::from_be_bytes([bytes[0], bytes[1]]))
            .collect::<Vec<_>>(),
    )
    .ok()
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
    fn png_round_trip_preserves_parameters() {
        let mut image = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut image, 1, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[1, 2, 3, 255]).unwrap();
        }
        let metadata = Metadata {
            parameters: "portrait, Steps: 20, Sampler: Euler".to_owned(),
        };
        let updated = embed(&image, &metadata).unwrap();
        assert_eq!(extract(&updated).unwrap(), Some(metadata));
    }

    #[test]
    fn webp_round_trip_preserves_pixels_and_unicode_parameters() {
        let mut image = Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(2, 2)
            .write_to(&mut image, image::ImageFormat::WebP)
            .unwrap();
        let image = image.into_inner();
        let metadata = Metadata {
            parameters: "ティファ portrait, Steps: 20".to_owned(),
        };
        let updated = embed(&image, &metadata).unwrap();
        assert_eq!(webp_pixels(&updated), webp_pixels(&image));
        assert_eq!(extract(&updated).unwrap(), Some(metadata));
    }
}
