use clap::Parser;
use image_convert::{EncodeOptions, OutputFormat, Transform};
use std::{
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

#[derive(Parser, Debug)]
#[command(
    name = "image-convert",
    version,
    about = "Convert exactly one supported image into one supported output"
)]
struct Args {
    input: PathBuf,
    output: PathBuf,
    #[arg(long)]
    overwrite: bool,
    #[arg(long)]
    width: Option<u32>,
    #[arg(long)]
    height: Option<u32>,
    #[arg(long, value_names=["X","Y","WIDTH","HEIGHT"], num_args=4)]
    crop: Option<Vec<u32>>,
    #[arg(long, default_value_t = 0)]
    rotate: u16,
    #[arg(long)]
    jpeg_quality: Option<u8>,
    #[arg(long)]
    webp_quality: Option<f32>,
    #[arg(long)]
    webp_lossless: bool,
    #[arg(long)]
    webp_method: Option<u8>,
    #[arg(long)]
    png_compression: Option<u8>,
}
fn main() -> ExitCode {
    match run_with_args(Args::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("image-convert: {e}");
            ExitCode::FAILURE
        }
    }
}
fn run_with_args(a: Args) -> Result<(), String> {
    if !a.input.is_file() {
        return Err(format!(
            "input does not exist or is not a file: {}",
            a.input.display()
        ));
    }
    if a.output.exists() && !a.overwrite {
        return Err(format!(
            "output already exists: {} (use --overwrite to replace it)",
            a.output.display()
        ));
    }
    if same_path(&a.input, &a.output)? {
        return Err("input and output must be different; the source is never overwritten".into());
    }
    let format = OutputFormat::from_path(&a.output).map_err(|e| e.to_string())?;
    validate_options(&a, format)?;
    let t = Transform {
        width: a.width,
        height: a.height,
        crop: a.crop.as_ref().map(|v| (v[0], v[1], v[2], v[3])),
        rotate: a.rotate,
    };
    let asset = image_convert::decode(&a.input)
        .and_then(|x| image_convert::transform(x, &t))
        .map_err(|e| e.to_string())?;
    let options = EncodeOptions {
        jpeg_quality: a.jpeg_quality,
        webp_quality: a.webp_quality,
        webp_lossless: a.webp_lossless,
        webp_method: a.webp_method,
        png_compression: a.png_compression,
    };
    transactional_write(
        &a.output,
        |candidate| {
            image_convert::encode(candidate, &asset, format, &options).map_err(|e| e.to_string())
        },
        |candidate| {
            let verified = image_convert::decode(candidate)
                .map_err(|e| format!("output image verification failed: {e}"))?;
            if asset_dimensions(&verified) != asset_dimensions(&asset) {
                return Err("output verification failed: image dimensions differ".into());
            }
            Ok(())
        },
    )
}

fn asset_dimensions(asset: &image_convert::ImageAsset) -> (u32, u32) {
    match asset {
        image_convert::ImageAsset::Still(i) => (i.width(), i.height()),
        image_convert::ImageAsset::Animation(a) => (a.width, a.height),
    }
}

fn transactional_write<E, V>(destination: &Path, encode: E, verify: V) -> Result<(), String>
where
    E: FnOnce(&Path) -> Result<(), String>,
    V: FnOnce(&Path) -> Result<(), String>,
{
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temp = tempfile::Builder::new()
        .prefix(".image-convert-")
        .suffix(".tmp")
        .tempfile_in(parent)
        .map_err(|e| format!("could not create temporary output: {e}"))?;
    let candidate = temp.into_temp_path();
    encode(&candidate)?;
    verify(&candidate)?;
    candidate
        .persist(destination)
        .map_err(|e| format!("could not commit verified output: {}", e.error))?;
    Ok(())
}
fn same_path(a: &Path, b: &Path) -> Result<bool, String> {
    let ca = fs::canonicalize(a).map_err(|e| e.to_string())?;
    let cb = if b.exists() {
        fs::canonicalize(b).map_err(|e| e.to_string())?
    } else {
        let parent = b
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::canonicalize(parent)
            .map_err(|e| format!("output directory does not exist: {e}"))?
            .join(b.file_name().ok_or("output needs a filename")?)
    };
    Ok(ca == cb)
}
fn validate_options(a: &Args, f: OutputFormat) -> Result<(), String> {
    if a.jpeg_quality.is_some() && f != OutputFormat::Jpeg {
        return Err("--jpeg-quality applies only to JPEG output".into());
    }
    if (a.webp_quality.is_some() || a.webp_lossless || a.webp_method.is_some())
        && f != OutputFormat::WebP
    {
        return Err("WebP options apply only to WebP output".into());
    }
    if a.png_compression.is_some() && f != OutputFormat::Png {
        return Err("--png-compression applies only to PNG output".into());
    }
    if a.jpeg_quality == Some(0) {
        return Err("--jpeg-quality must be between 1 and 100".into());
    }
    if a.webp_quality.is_some_and(|v| !(0.0..=100.0).contains(&v)) {
        return Err("--webp-quality must be between 0 and 100".into());
    }
    if a.webp_method.is_some_and(|v| v > 6) {
        return Err("--webp-method must be between 0 and 6".into());
    }
    if a.png_compression.is_some_and(|v| v > 9) {
        return Err("--png-compression must be between 0 and 9".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use image::{DynamicImage, Rgba, RgbaImage};
    #[test]
    fn rejects_irrelevant_codec_option() {
        let a = Args::try_parse_from(["image-convert", "in.png", "out.jpg", "--webp-lossless"])
            .unwrap();
        assert!(
            validate_options(&a, OutputFormat::Jpeg)
                .unwrap_err()
                .contains("WebP")
        );
    }

    #[test]
    fn verification_failure_preserves_existing_destination() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("existing.png");
        fs::write(&output, b"original").unwrap();
        let error = transactional_write(
            &output,
            |p| fs::write(p, b"candidate").map_err(|e| e.to_string()),
            |_| Err("forced verification failure".into()),
        )
        .unwrap_err();
        assert!(error.contains("forced"));
        assert_eq!(fs::read(&output).unwrap(), b"original");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn overwrite_commits_only_the_verified_candidate() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("in.png");
        let output = dir.path().join("out.png");
        let asset = image_convert::ImageAsset::Still(DynamicImage::ImageRgba8(
            RgbaImage::from_pixel(1, 1, Rgba([9, 8, 7, 255])),
        ));
        image_convert::encode(&input, &asset, OutputFormat::Png, &Default::default()).unwrap();
        fs::write(&output, b"old destination").unwrap();
        let args = Args::try_parse_from([
            "image-convert",
            input.to_str().unwrap(),
            output.to_str().unwrap(),
            "--overwrite",
        ])
        .unwrap();
        run_with_args(args).unwrap();
        assert!(fs::read(&output).unwrap().starts_with(b"\x89PNG"));
    }
}
