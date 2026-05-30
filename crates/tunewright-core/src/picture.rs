use crate::types::TunewrightError;
use image::imageops::FilterType;
use lofty::config::{ParseOptions, WriteOptions};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::probe::Probe;
use std::io::Cursor;
use std::path::Path;

/// Extract the first embedded cover art from an audio file
pub fn extract_cover_art(path: &Path) -> Result<Option<(Vec<u8>, String)>, TunewrightError> {
    // We only want the cover picture, not audio properties — skip parsing those.
    let tagged = Probe::open(path)
        .map_err(|e| TunewrightError::TagReadError(format!("{}: {}", path.display(), e)))?
        .options(ParseOptions::new().read_properties(false))
        .read()
        .map_err(|e| TunewrightError::TagReadError(format!("{}: {}", path.display(), e)))?;

    for tag in tagged.tags() {
        if let Some(pic) = tag.pictures().first() {
            let mime = match pic.mime_type() {
                Some(MimeType::Jpeg) => "image/jpeg",
                Some(MimeType::Png) => "image/png",
                Some(MimeType::Bmp) => "image/bmp",
                Some(MimeType::Gif) => "image/gif",
                Some(MimeType::Tiff) => "image/tiff",
                _ => "image/jpeg",
            };
            return Ok(Some((pic.data().to_vec(), mime.to_string())));
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
        Some((data, _mime)) => {
            let img = image::load_from_memory(&data)
                .map_err(|e| TunewrightError::ImageError(e.to_string()))?;

            if img.width() <= max_size && img.height() <= max_size {
                // Already small enough, return as JPEG
                let mut buf = Vec::new();
                let mut cursor = Cursor::new(&mut buf);
                img.write_to(&mut cursor, image::ImageFormat::Jpeg)
                    .map_err(|e| TunewrightError::ImageError(e.to_string()))?;
                return Ok(Some((buf, "image/jpeg".to_string())));
            }

            let thumb = img.resize(max_size, max_size, FilterType::Lanczos3);
            let mut buf = Vec::new();
            let mut cursor = Cursor::new(&mut buf);
            thumb
                .write_to(&mut cursor, image::ImageFormat::Jpeg)
                .map_err(|e| TunewrightError::ImageError(e.to_string()))?;

            Ok(Some((buf, "image/jpeg".to_string())))
        }
    }
}

/// Embed cover art into an audio file
pub fn embed_cover_art(path: &Path, image_data: &[u8]) -> Result<(), TunewrightError> {
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
        .unwrap_or(lofty::tag::TagType::Id3v2);

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
