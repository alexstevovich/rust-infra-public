//! Still-image and animation decoding, transformation, and encoding.

use image::{AnimationDecoder, DynamicImage, ImageFormat, RgbaImage, imageops::FilterType};
use std::{
    fs,
    io::{BufReader, BufWriter, Cursor},
    path::Path,
};
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct AnimationFrame {
    pub pixels: RgbaImage,
    pub duration_ms: u32,
}
#[derive(Clone, Debug)]
pub struct Animation {
    pub width: u32,
    pub height: u32,
    pub frames: Vec<AnimationFrame>,
    pub loop_count: u16,
}
#[derive(Clone, Debug)]
pub enum ImageAsset {
    Still(DynamicImage),
    Animation(Animation),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    Png,
    Jpeg,
    WebP,
    Gif,
}
impl OutputFormat {
    pub fn from_path(path: &Path) -> Result<Self, Error> {
        match path
            .extension()
            .and_then(|x| x.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("png") => Ok(Self::Png),
            Some("jpg" | "jpeg") => Ok(Self::Jpeg),
            Some("webp") => Ok(Self::WebP),
            Some("gif") => Ok(Self::Gif),
            _ => Err(Error::UnsupportedOutput),
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct Transform {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub crop: Option<(u32, u32, u32, u32)>,
    pub rotate: u16,
}
#[derive(Clone, Debug)]
pub struct EncodeOptions {
    pub jpeg_quality: Option<u8>,
    pub webp_quality: Option<f32>,
    pub webp_lossless: bool,
    pub webp_method: Option<u8>,
    pub png_compression: Option<u8>,
}
impl Default for EncodeOptions {
    fn default() -> Self {
        Self {
            jpeg_quality: None,
            webp_quality: None,
            webp_lossless: false,
            webp_method: None,
            png_compression: None,
        }
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("image codec error: {0}")]
    Image(#[from] image::ImageError),
    #[error("GIF error: {0}")]
    Gif(String),
    #[error("WebP animation error: {0}")]
    WebP(String),
    #[error("unsupported input format")]
    UnsupportedInput,
    #[error("unsupported output format (use .png, .jpg/.jpeg, .webp, or .gif)")]
    UnsupportedOutput,
    #[error("animated input cannot be written to a still-only format")]
    AnimationToStill,
    #[error("invalid transform: {0}")]
    Transform(String),
}

pub fn decode(path: &Path) -> Result<ImageAsset, Error> {
    let bytes = fs::read(path)?;
    let format = image::guess_format(&bytes).map_err(|_| Error::UnsupportedInput)?;
    match format {
        ImageFormat::Gif => decode_gif(&bytes),
        ImageFormat::WebP if is_animated_webp(&bytes) => decode_webp(&bytes),
        ImageFormat::Png
        | ImageFormat::Jpeg
        | ImageFormat::WebP
        | ImageFormat::Bmp
        | ImageFormat::Tiff => Ok(ImageAsset::Still(image::load_from_memory_with_format(
            &bytes, format,
        )?)),
        _ => Err(Error::UnsupportedInput),
    }
}

fn is_animated_webp(b: &[u8]) -> bool {
    b.windows(4).any(|x| x == b"ANIM")
}
fn decode_gif(bytes: &[u8]) -> Result<ImageAsset, Error> {
    let raw = gif::DecodeOptions::new()
        .read_info(bytes)
        .map_err(|e| Error::Gif(e.to_string()))?;
    let (w, h) = (raw.width() as u32, raw.height() as u32);
    let loop_count = match raw.repeat() {
        gif::Repeat::Infinite => 0,
        gif::Repeat::Finite(n) => n,
    };
    drop(raw);
    let decoder = image::codecs::gif::GifDecoder::new(BufReader::new(Cursor::new(bytes)))?;
    let frames = decoder
        .into_frames()
        .collect_frames()?
        .into_iter()
        .map(|f| {
            let (n, d) = f.delay().numer_denom_ms();
            AnimationFrame {
                pixels: f.into_buffer(),
                duration_ms: (n / d.max(1)).max(10),
            }
        })
        .collect();
    Ok(ImageAsset::Animation(Animation {
        width: w,
        height: h,
        frames,
        loop_count,
    }))
}
fn decode_webp(bytes: &[u8]) -> Result<ImageAsset, Error> {
    let d = webp_animation::Decoder::new(bytes).map_err(|e| Error::WebP(e.to_string()))?;
    let info = d.dimensions();
    let loop_count = webp_loop_count(bytes);
    let mut last = 0;
    let frames = d
        .into_iter()
        .map(|f| {
            let ts = f.timestamp();
            let duration_ms = (ts - last).max(1) as u32;
            last = ts;
            let pixels = RgbaImage::from_raw(info.0, info.1, f.data().to_vec())
                .ok_or_else(|| Error::WebP("invalid frame buffer".into()))?;
            Ok(AnimationFrame {
                pixels,
                duration_ms,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(ImageAsset::Animation(Animation {
        width: info.0,
        height: info.1,
        frames,
        loop_count: loop_count as u16,
    }))
}
fn webp_loop_count(bytes: &[u8]) -> u16 {
    bytes
        .windows(4)
        .position(|x| x == b"ANIM")
        .and_then(|p| bytes.get(p + 12..p + 14))
        .map(|x| u16::from_le_bytes([x[0], x[1]]))
        .unwrap_or(0)
}
pub fn transform(asset: ImageAsset, t: &Transform) -> Result<ImageAsset, Error> {
    match asset {
        ImageAsset::Still(i) => Ok(ImageAsset::Still(apply(i, t)?)),
        ImageAsset::Animation(mut a) => {
            for f in &mut a.frames {
                f.pixels =
                    apply(DynamicImage::ImageRgba8(std::mem::take(&mut f.pixels)), t)?.to_rgba8();
            }
            if let Some(f) = a.frames.first() {
                a.width = f.pixels.width();
                a.height = f.pixels.height();
            }
            Ok(ImageAsset::Animation(a))
        }
    }
}
fn apply(mut i: DynamicImage, t: &Transform) -> Result<DynamicImage, Error> {
    if let Some((x, y, w, h)) = t.crop {
        if w == 0
            || h == 0
            || x.checked_add(w).is_none_or(|right| right > i.width())
            || y.checked_add(h).is_none_or(|bottom| bottom > i.height())
        {
            return Err(Error::Transform("crop lies outside the image".into()));
        }
        i = i.crop_imm(x, y, w, h);
    }
    if t.width.is_some() || t.height.is_some() {
        let (ow, oh) = (i.width(), i.height());
        let (w, h) = match (t.width, t.height) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) => (w, (oh as u64 * w as u64 / ow as u64) as u32),
            (None, Some(h)) => ((ow as u64 * h as u64 / oh as u64) as u32, h),
            _ => (ow, oh),
        };
        if w == 0 || h == 0 {
            return Err(Error::Transform("dimensions must be nonzero".into()));
        }
        i = i.resize_exact(w, h, FilterType::Lanczos3);
    }
    Ok(match t.rotate {
        0 => i,
        90 => i.rotate90(),
        180 => i.rotate180(),
        270 => i.rotate270(),
        _ => {
            return Err(Error::Transform(
                "rotation must be 0, 90, 180, or 270".into(),
            ));
        }
    })
}

pub fn encode(
    path: &Path,
    asset: &ImageAsset,
    format: OutputFormat,
    o: &EncodeOptions,
) -> Result<(), Error> {
    let result = match asset {
        ImageAsset::Animation(_) if matches!(format, OutputFormat::Png | OutputFormat::Jpeg) => {
            Err(Error::AnimationToStill)
        }
        ImageAsset::Animation(a) => encode_animation(path, a, format, o),
        ImageAsset::Still(i) => encode_still(path, i, format, o),
    };
    result
}
fn encode_still(
    path: &Path,
    i: &DynamicImage,
    f: OutputFormat,
    o: &EncodeOptions,
) -> Result<(), Error> {
    if f == OutputFormat::Png && o.png_compression.is_some() {
        let file = fs::File::create(path)?;
        let mut e = png::Encoder::new(BufWriter::new(file), i.width(), i.height());
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        if let Some(level) = o.png_compression {
            e.set_compression(if level <= 2 {
                png::Compression::Fast
            } else if level <= 6 {
                png::Compression::Balanced
            } else {
                png::Compression::High
            });
        }
        let mut w = e
            .write_header()
            .map_err(|x| Error::Io(std::io::Error::other(x)))?;
        w.write_image_data(&i.to_rgba8())
            .map_err(|x| Error::Io(std::io::Error::other(x)))?;
        return Ok(());
    }
    if f == OutputFormat::WebP {
        return encode_webp_frames(path, i.width(), i.height(), &[(i.to_rgba8(), 1)], 0, o);
    }
    let mut file = BufWriter::new(fs::File::create(path)?);
    match f {
        OutputFormat::Jpeg => {
            let mut e = image::codecs::jpeg::JpegEncoder::new_with_quality(
                &mut file,
                o.jpeg_quality.unwrap_or(85),
            );
            e.encode_image(i)?
        }
        OutputFormat::Png => i.write_to(&mut file, ImageFormat::Png)?,
        OutputFormat::Gif => i.write_to(&mut file, ImageFormat::Gif)?,
        OutputFormat::WebP => unreachable!(),
    };
    Ok(())
}
fn encode_animation(
    path: &Path,
    a: &Animation,
    f: OutputFormat,
    o: &EncodeOptions,
) -> Result<(), Error> {
    match f {
        OutputFormat::Gif => {
            let file = fs::File::create(path)?;
            let mut e =
                gif::Encoder::new(BufWriter::new(file), a.width as u16, a.height as u16, &[])
                    .map_err(|x| Error::Gif(x.to_string()))?;
            e.set_repeat(if a.loop_count == 0 {
                gif::Repeat::Infinite
            } else {
                gif::Repeat::Finite(a.loop_count)
            })
            .map_err(|x| Error::Gif(x.to_string()))?;
            for fr in &a.frames {
                let mut b = fr.pixels.clone().into_raw();
                let mut gf =
                    gif::Frame::from_rgba_speed(a.width as u16, a.height as u16, &mut b, 10);
                gf.delay = ((fr.duration_ms + 5) / 10).max(1) as u16;
                e.write_frame(&gf).map_err(|x| Error::Gif(x.to_string()))?;
            }
            Ok(())
        }
        OutputFormat::WebP => {
            let frames = a
                .frames
                .iter()
                .map(|f| (f.pixels.clone(), f.duration_ms))
                .collect::<Vec<_>>();
            encode_webp_frames(path, a.width, a.height, &frames, a.loop_count, o)
        }
        _ => Err(Error::AnimationToStill),
    }
}
fn encode_webp_frames(
    path: &Path,
    width: u32,
    height: u32,
    frames: &[(RgbaImage, u32)],
    loop_count: u16,
    o: &EncodeOptions,
) -> Result<(), Error> {
    use webp_animation::{AnimParams, EncoderOptions, EncodingConfig, EncodingType};
    let config = EncodingConfig {
        encoding_type: if o.webp_lossless {
            EncodingType::Lossless
        } else {
            EncodingType::new_lossy()
        },
        quality: o
            .webp_quality
            .unwrap_or(if o.webp_lossless { 75.0 } else { 85.0 }),
        method: o.webp_method.unwrap_or(4) as usize,
    };
    let options = EncoderOptions {
        anim_params: AnimParams {
            loop_count: loop_count as i32,
        },
        encoding_config: Some(config),
        ..Default::default()
    };
    let mut enc = webp_animation::Encoder::new_with_options((width, height), options)
        .map_err(|x| Error::WebP(x.to_string()))?;
    let mut ts = 0;
    for (fr, duration) in frames {
        enc.add_frame(fr, ts)
            .map_err(|x| Error::WebP(x.to_string()))?;
        ts += *duration as i32;
    }
    let data = enc.finalize(ts).map_err(|x| Error::WebP(x.to_string()))?;
    fs::write(path, &*data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn still() -> ImageAsset {
        ImageAsset::Still(DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            3,
            2,
            image::Rgba([20, 40, 60, 255]),
        )))
    }
    fn animation() -> ImageAsset {
        ImageAsset::Animation(Animation {
            width: 2,
            height: 2,
            loop_count: 3,
            frames: vec![
                AnimationFrame {
                    pixels: RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255])),
                    duration_ms: 100,
                },
                AnimationFrame {
                    pixels: RgbaImage::from_pixel(2, 2, image::Rgba([0, 0, 255, 255])),
                    duration_ms: 200,
                },
            ],
        })
    }
    fn webp_chunk_names(bytes: &[u8]) -> Vec<[u8; 4]> {
        assert!(bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP");
        let mut names = Vec::new();
        let mut position = 12usize;
        while position + 8 <= bytes.len() {
            let name = bytes[position..position + 4].try_into().unwrap();
            let length =
                u32::from_le_bytes(bytes[position + 4..position + 8].try_into().unwrap()) as usize;
            names.push(name);
            position += 8 + length + (length & 1);
        }
        names
    }

    #[test]
    fn still_webp_is_not_an_animation_container() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("still.webp");
        encode(&path, &still(), OutputFormat::WebP, &Default::default()).unwrap();
        let bytes = fs::read(&path).unwrap();
        let chunks = webp_chunk_names(&bytes);
        assert!(!chunks.contains(b"ANIM"));
        assert!(!chunks.contains(b"ANMF"));
        assert!(matches!(decode(&path).unwrap(), ImageAsset::Still(_)));
    }
    #[test]
    fn still_conversions_and_resize() {
        let d = tempfile::tempdir().unwrap();
        for (f, n) in [
            (OutputFormat::Png, "x.png"),
            (OutputFormat::Jpeg, "x.jpg"),
            (OutputFormat::WebP, "x.webp"),
        ] {
            let p = d.path().join(n);
            encode(&p, &still(), f, &Default::default()).unwrap();
            assert!(p.metadata().unwrap().len() > 0);
        }
        let out = transform(
            still(),
            &Transform {
                width: Some(6),
                height: None,
                ..Default::default()
            },
        )
        .unwrap();
        match out {
            ImageAsset::Still(i) => assert_eq!((i.width(), i.height()), (6, 4)),
            _ => panic!(),
        }
    }
    #[test]
    fn animation_gif_webp_roundtrip_and_still_rejection() {
        let d = tempfile::tempdir().unwrap();
        let gif = d.path().join("a.gif");
        encode(&gif, &animation(), OutputFormat::Gif, &Default::default()).unwrap();
        let decoded = decode(&gif).unwrap();
        assert!(matches!(&decoded,ImageAsset::Animation(a) if a.frames.len()==2));
        let webp = d.path().join("a.webp");
        encode(&webp, &decoded, OutputFormat::WebP, &Default::default()).unwrap();
        let wd = decode(&webp).unwrap();
        let chunks = webp_chunk_names(&fs::read(&webp).unwrap());
        assert!(chunks.contains(b"ANIM"));
        assert!(chunks.contains(b"ANMF"));
        assert!(matches!(&wd,ImageAsset::Animation(a) if a.frames.len()==2 && a.loop_count==3));
        let gif2 = d.path().join("b.gif");
        encode(&gif2, &wd, OutputFormat::Gif, &Default::default()).unwrap();
        assert!(matches!(decode(&gif2).unwrap(),ImageAsset::Animation(a) if a.frames.len()==2));
        assert!(matches!(
            encode(
                &d.path().join("x.jpg"),
                &animation(),
                OutputFormat::Jpeg,
                &Default::default()
            ),
            Err(Error::AnimationToStill)
        ));
    }
}
