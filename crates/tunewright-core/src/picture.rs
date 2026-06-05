use crate::types::TunewrightError;
use image::imageops::FilterType;
use lofty::config::{ParseOptions, WriteOptions};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::probe::Probe;
use std::io::Cursor;
use std::path::Path;

fn detect_mime(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(&[0xFF, 0xD8]) {
        Some("image/jpeg")
    } else if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        Some("image/png")
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if data.starts_with(b"BM") {
        Some("image/bmp")
    } else if data.starts_with(&[0x49, 0x49, 0x2A, 0x00]) || data.starts_with(&[0x4D, 0x4D, 0x00, 0x2A]) {
        Some("image/tiff")
    } else if data.starts_with(b"RIFF") && data.len() > 12 && &data[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// Extract the first embedded cover art from an audio file
pub fn extract_cover_art(path: &Path) -> Result<Option<(Vec<u8>, String)>, TunewrightError> {
    // We only want the cover picture, not audio properties — skip parsing those.
    let tagged = Probe::open(path)
        .map_err(|e| TunewrightError::TagReadError(format!("{}: {}", path.display(), e)))?
        .options(ParseOptions::new().read_properties(false))
        .read()
        .map_err(|e| TunewrightError::TagReadError(format!("{}: {}", path.display(), e)))?;

    for tag in tagged.tags() {
        let pic = tag
            .pictures()
            .iter()
            .find(|p| p.pic_type() == PictureType::CoverFront)
            .or_else(|| tag.pictures().first());

        if let Some(pic) = pic {
            let mime = match pic.mime_type() {
                Some(mime_type) => {
                    let s = mime_type.as_str();
                    if s.is_empty() {
                        detect_mime(pic.data()).unwrap_or("image/jpeg").to_string()
                    } else {
                        s.to_string()
                    }
                }
                None => detect_mime(pic.data()).unwrap_or("image/jpeg").to_string(),
            };
            return Ok(Some((pic.data().to_vec(), mime)));
        }
    }

    Ok(None)
}

/// Extract cover art and optionally resize to a thumbnail
pub fn extract_cover_art_thumbnail(
    path: &Path,
    max_size: u32,
) -> Result<Option<(Vec<u8>, String)>, TunewrightError> {
    let art = extract_cover_art(path)?;
    match art {
        None => Ok(None),
        Some((data, mime)) if max_size == 0 => Ok(Some((data, mime))),
        Some((data, mime)) => {
            let img = image::load_from_memory(&data)
                .map_err(|e| TunewrightError::ImageError(e.to_string()))?;

            if img.width() <= max_size && img.height() <= max_size {
                // Already small enough, return as-is to avoid quality / metadata / format loss
                return Ok(Some((data, mime)));
            }

            let thumb = img.resize(max_size, max_size, FilterType::Lanczos3);
            let mut buf = Vec::new();
            let mut cursor = Cursor::new(&mut buf);

            // Select encoding format depending on source format or alpha channel presence
            let (format, out_mime) = if mime == "image/png" || thumb.has_alpha() {
                (image::ImageFormat::Png, "image/png")
            } else {
                (image::ImageFormat::Jpeg, "image/jpeg")
            };

            thumb
                .write_to(&mut cursor, format)
                .map_err(|e| TunewrightError::ImageError(e.to_string()))?;

            Ok(Some((buf, out_mime.to_string())))
        }
    }
}

/// Embed cover art into an audio file
pub fn embed_cover_art(path: &Path, image_data: &[u8]) -> Result<(), TunewrightError> {
    let _lock = crate::locks::lock_file(path);
    let mut tagged = Probe::open(path)
        .map_err(|e| TunewrightError::TagWriteError(format!("{}: {}", path.display(), e)))?
        .read()
        .map_err(|e| TunewrightError::TagWriteError(format!("{}: {}", path.display(), e)))?;

    // Detect mime type
    let mime = if image_data.starts_with(&[0xFF, 0xD8]) {
        MimeType::Jpeg
    } else if image_data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        MimeType::Png
    } else {
        MimeType::Jpeg // fallback
    };

    let picture = Picture::unchecked(image_data.to_vec())
        .pic_type(PictureType::CoverFront)
        .mime_type(mime)
        .build();

    // Get primary tag type
    let primary_type = tagged
        .primary_tag()
        .map(|t| t.tag_type())
        .unwrap_or_else(|| tagged.primary_tag_type());

    let tag = match tagged.tag_mut(primary_type) {
        Some(t) => t,
        None => {
            tagged.insert_tag(lofty::tag::Tag::new(primary_type));
            tagged.tag_mut(primary_type).unwrap()
        }
    };

    // Remove existing cover art and add new
    tag.remove_picture_type(PictureType::CoverFront);
    tag.push_picture(picture);

    tagged
        .save_to_path(path, WriteOptions::default())
        .map_err(|e| TunewrightError::TagWriteError(format!("{}: {}", path.display(), e)))?;

    Ok(())
}

/// Remove all cover art from an audio file
pub fn remove_cover_art(path: &Path) -> Result<(), TunewrightError> {
    let _lock = crate::locks::lock_file(path);
    let mut tagged = Probe::open(path)
        .map_err(|e| TunewrightError::TagWriteError(format!("{}: {}", path.display(), e)))?
        .read()
        .map_err(|e| TunewrightError::TagWriteError(format!("{}: {}", path.display(), e)))?;

    let primary_type = tagged
        .primary_tag()
        .map(|t| t.tag_type())
        .unwrap_or(lofty::tag::TagType::Id3v2);

    if let Some(tag) = tagged.tag_mut(primary_type) {
        // Remove all pictures
        while !tag.pictures().is_empty() {
            tag.remove_picture(0);
        }
    }

    tagged
        .save_to_path(path, WriteOptions::default())
        .map_err(|e| TunewrightError::TagWriteError(format!("{}: {}", path.display(), e)))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    fn rand_num() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::{SystemTime, UNIX_EPOCH};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        nanos.wrapping_add(count)
    }

    #[test]
    fn test_detect_mime() {
        assert_eq!(detect_mime(&[0xFF, 0xD8, 0x00]), Some("image/jpeg"));
        assert_eq!(detect_mime(&[0x89, 0x50, 0x4E, 0x47]), Some("image/png"));
        assert_eq!(detect_mime(b"GIF89a..."), Some("image/gif"));
        assert_eq!(detect_mime(b"BM..."), Some("image/bmp"));
        assert_eq!(detect_mime(b"RIFF\x00\x00\x00\x00WEBP..."), Some("image/webp"));
        assert_eq!(detect_mime(&[0x00, 0x01]), None);
    }

    #[test]
    fn test_extract_cover_art_mime_detection() {
        let temp_dir = std::env::temp_dir().join(format!("tunewright_test_{}", rand_num()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let audio_path = temp_dir.join("test.flac");

        // Create a dummy flac file
        let flac_bytes = b"fLaC\x80\x00\x00\x22\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";
        File::create(&audio_path).unwrap().write_all(flac_bytes).unwrap();

        // 1. JPEG image data
        let jpeg_data = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46];
        embed_cover_art(&audio_path, &jpeg_data).unwrap();

        let art = extract_cover_art(&audio_path).unwrap().unwrap();
        assert_eq!(art.1, "image/jpeg");

        // 2. PNG image data
        let png_data = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        embed_cover_art(&audio_path, &png_data).unwrap();

        let art = extract_cover_art(&audio_path).unwrap().unwrap();
        assert_eq!(art.1, "image/png");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
