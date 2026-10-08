//! Thumbnails for list and grid (spec 044 FR-005, research R7): decoded in Rust with limits,
//! turned upright by their EXIF orientation, shrunk to [`EDGE`] and cached as JPEG in holzi's cache
//! (never synced, ADR 0001; an own place, so agents never reach it). The key covers source, path,
//! size and modification time, so a changed file gets a new thumbnail. An image that fails leaves a
//! marker and is not tried again until it changes.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use fast_image_resize::images::Image;
use fast_image_resize::{IntoImageView, PixelType, ResizeOptions, Resizer};
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageDecoder, ImageReader, Limits, RgbImage};
use sha2::{Digest, Sha256};

use crate::files::{FilesError, FilesErrorCode};

/// The longest edge of a thumbnail.
pub const EDGE: u32 = 320;

/// JPEG quality of a thumbnail.
const QUALITY: u8 = 80;

/// Largest width or height a thumbnail is made from.
const MAX_SIDE: u32 = 20_000;

/// Most memory one decode may take.
const MAX_ALLOC: u64 = 512 * 1024 * 1024;

/// The folder below holzi's cache directory.
pub const CACHE_FOLDER: &str = "files-thumbnails";

/// The cache key of a thumbnail: hex SHA-256 over source, path, size and modification time.
pub fn cache_key(source: &str, path: &str, size: u64, modified_ms: i64) -> String {
    let mut hasher = Sha256::new();
    for part in [
        source.as_bytes(),
        path.as_bytes(),
        &size.to_le_bytes(),
        &modified_ms.to_le_bytes(),
    ] {
        hasher.update((part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The JPEG thumbnail of the image at `path`, from the cache in `cache_dir` or made now.
pub fn thumbnail(
    cache_dir: &Path,
    source: &str,
    path: &Path,
    size: u64,
    modified_ms: i64,
) -> Result<Vec<u8>, FilesError> {
    let key = cache_key(source, &path.to_string_lossy(), size, modified_ms);
    let cached = cache_dir.join(format!("{key}.jpg"));
    let broken = cache_dir.join(format!("{key}.broken"));
    if let Ok(bytes) = std::fs::read(&cached) {
        return Ok(bytes);
    }
    if broken.exists() {
        return Err(broken_error());
    }
    match render(path) {
        Ok(bytes) => {
            if std::fs::create_dir_all(cache_dir).is_ok() {
                if let Err(error) = write_atomically(&cached, &bytes) {
                    log::warn!("files: could not cache a thumbnail: {error}");
                }
            }
            Ok(bytes)
        }
        Err(reason) => {
            log::info!("files: no thumbnail for {}: {reason}", path.display());
            if std::fs::create_dir_all(cache_dir).is_ok() {
                let _ = std::fs::write(&broken, b"");
            }
            Err(broken_error())
        }
    }
}

fn broken_error() -> FilesError {
    FilesError::new(FilesErrorCode::Broken, "no thumbnail for this image")
}

fn write_atomically(target: &PathBuf, bytes: &[u8]) -> std::io::Result<()> {
    let part = target.with_extension("jpg.part");
    std::fs::write(&part, bytes)?;
    std::fs::rename(part, target)
}

fn render(path: &Path) -> Result<Vec<u8>, String> {
    let mut reader = ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_ALLOC);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    image.apply_orientation(orientation);
    let rgb = DynamicImage::ImageRgb8(image.to_rgb8());
    let (width, height) = fit(rgb.width(), rgb.height());
    let small = if (width, height) == (rgb.width(), rgb.height()) {
        rgb.to_rgb8()
    } else {
        let mut target = Image::new(width, height, PixelType::U8x3);
        if rgb.pixel_type() != Some(PixelType::U8x3) {
            return Err("unexpected pixel type".to_owned());
        }
        Resizer::new()
            .resize(&rgb, &mut target, &ResizeOptions::new())
            .map_err(|e| e.to_string())?;
        RgbImage::from_raw(width, height, target.into_vec())
            .ok_or_else(|| "resized buffer has the wrong size".to_owned())?
    };
    let mut bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut Cursor::new(&mut bytes), QUALITY)
        .encode_image(&small)
        .map_err(|e| e.to_string())?;
    Ok(bytes)
}

/// The size within [`EDGE`] × [`EDGE`] keeping the shape; never larger than the original.
fn fit(width: u32, height: u32) -> (u32, u32) {
    let longest = width.max(height);
    if longest <= EDGE || longest == 0 {
        return (width, height);
    }
    let scale =
        |side: u32| ((u64::from(side) * u64::from(EDGE)) / u64::from(longest)).max(1) as u32;
    (scale(width), scale(height))
}

#[cfg(test)]
#[path = "thumbnails_tests.rs"]
mod tests;
