use crate::types::{TagData, TagWriteChanges, TunewrightError, WriteResult};
use lofty::aac::AacFile;
use lofty::ape::{ApeFile, ApeItem, ApeTag};
use lofty::config::{ParseOptions, ParsingMode, WriteOptions};
use lofty::file::{AudioFile, FileType, TaggedFile, TaggedFileExt};
use lofty::flac::FlacFile;
use lofty::id3::v2::{ExtendedTextFrame, Frame, Id3v2Tag, Id3v2Version};
use lofty::iff::aiff::AiffFile;
use lofty::iff::wav::{RiffInfoList, WavFile};
use lofty::mp4::{Atom, AtomData, AtomIdent, Ilst};
use lofty::mpeg::MpegFile;
use lofty::musepack::MpcFile;
use lofty::ogg::tag::VorbisComments;
use lofty::ogg::{OggPictureStorage, OpusFile, SpeexFile, VorbisFile};
use lofty::probe::Probe;
use lofty::tag::{Accessor, ItemKey, ItemValue, MergeTag, SplitTag, Tag, TagExt, TagItem, TagType};
use lofty::wavpack::WavPackFile;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Fast parse options: tags only, no audio properties, no cover art data.
/// This only reads the tag headers (first few KB of the file).
fn fast_parse_options() -> ParseOptions {
    ParseOptions::new()
        .read_properties(false)
        .read_cover_art(false)
        .parsing_mode(ParsingMode::Relaxed)
}

/// Full parse options: everything including audio properties.
/// Needed for duration, bitrate, sample rate.
fn full_parse_options() -> ParseOptions {
    ParseOptions::new()
        .read_properties(true)
        .read_cover_art(false) // still skip loading cover art bytes
        .parsing_mode(ParsingMode::BestAttempt)
}

pub(crate) fn probe(
    path: &Path,
    options: ParseOptions,
) -> Result<Probe<std::io::BufReader<std::fs::File>>, Box<dyn std::error::Error>> {
    let probe = Probe::open(path)?.options(options).guess_file_type()?;
    let mut magic = [0u8; 4];
    let is_ogg = std::io::Read::read_exact(&mut std::fs::File::open(path)?, &mut magic).is_ok()
        && &magic == b"OggS";
    if is_ogg
        && !matches!(
            probe.file_type(),
            Some(FileType::Vorbis | FileType::Opus | FileType::Speex)
        )
    {
        return Err("unsupported Ogg stream".into());
    }
    Ok(probe)
}

/// Read tags FAST — skips audio properties and cover art data.
/// Returns tag text fields only (title, artist, album, etc.).
/// Use this for populating the grid quickly.
pub fn read_tags_fast(path: &Path) -> Result<TagData, TunewrightError> {
    read_tags_fast_with(path, false)
}

pub fn read_tags_fast_with(path: &Path, unfiltered: bool) -> Result<TagData, TunewrightError> {
    let tagged = probe(path, fast_parse_options())
        .map_err(|e| TunewrightError::TagReadError(format!("{}: {}", path.display(), e)))?
        .read()
        .map_err(|e| TunewrightError::TagReadError(format!("{}: {}", path.display(), e)))?;

    let tag_types: Vec<String> = tagged
        .tags()
        .iter()
        .map(|t| format!("{:?}", t.tag_type()))
        .collect();

    let tags = ordered_tags(&tagged);

    let title = first_string(&tags, |t| t.title());
    let artist = first_string(&tags, |t| t.artist());
    let album = first_string(&tags, |t| t.album());
    let genre = first_string(&tags, |t| t.genre());
    let comment = first_string(&tags, |t| t.comment());
    let year = tags.iter().find_map(|t| read_year(t));
    let track_number = tags.iter().find_map(|t| t.track());
    let track_total = tags.iter().find_map(|t| t.track_total());
    let disc_number = tags.iter().find_map(|t| t.disk());
    let disc_total = tags.iter().find_map(|t| t.disk_total());

    let album_artist = first_item_value(&tags, ItemKey::AlbumArtist);
    let composer = first_item_value(&tags, ItemKey::Composer);

    // Check picture count without loading picture data
    let has_cover = tags.iter().any(|t| !t.pictures().is_empty());

    let extra = collect_extra_tags(
        &tags,
        unfiltered,
        !unfiltered,
        primary_custom(path, &tagged),
    );

    Ok(TagData {
        title,
        artist,
        album,
        album_artist,
        year,
        track_number,
        track_total,
        disc_number,
        disc_total,
        genre,
        comment,
        composer,
        // Audio properties not available in fast mode
        bitrate: None,
        sample_rate: None,
        channels: None,
        duration_secs: None,
        format: Some(format!("{:?}", tagged.file_type())),
        tag_types,
        has_cover,
        extra,
    })
}

/// Read tags with full audio properties (duration, bitrate, sample rate).
/// Slower — use for detailed view or when user explicitly requests properties.
pub fn read_tags_full(path: &Path) -> Result<TagData, TunewrightError> {
    let tagged = probe(path, full_parse_options())
        .map_err(|e| TunewrightError::TagReadError(format!("{}: {}", path.display(), e)))?
        .read()
        .map_err(|e| TunewrightError::TagReadError(format!("{}: {}", path.display(), e)))?;

    let props = tagged.properties();
    let duration = props.duration();
    let duration_secs = if duration.as_secs() > 0 || duration.subsec_millis() > 0 {
        Some(duration.as_secs_f64())
    } else {
        None
    };

    let tag_types: Vec<String> = tagged
        .tags()
        .iter()
        .map(|t| format!("{:?}", t.tag_type()))
        .collect();

    let tags = ordered_tags(&tagged);

    let title = first_string(&tags, |t| t.title());
    let artist = first_string(&tags, |t| t.artist());
    let album = first_string(&tags, |t| t.album());
    let genre = first_string(&tags, |t| t.genre());
    let comment = first_string(&tags, |t| t.comment());
    let year = tags.iter().find_map(|t| read_year(t));
    let track_number = tags.iter().find_map(|t| t.track());
    let track_total = tags.iter().find_map(|t| t.track_total());
    let disc_number = tags.iter().find_map(|t| t.disk());
    let disc_total = tags.iter().find_map(|t| t.disk_total());

    let album_artist = first_item_value(&tags, ItemKey::AlbumArtist);
    let composer = first_item_value(&tags, ItemKey::Composer);

    let has_cover = tags.iter().any(|t| !t.pictures().is_empty());

    let extra = collect_extra_tags(&tags, true, true, primary_custom(path, &tagged));

    Ok(TagData {
        title,
        artist,
        album,
        album_artist,
        year,
        track_number,
        track_total,
        disc_number,
        disc_total,
        genre,
        comment,
        composer,
        bitrate: Some(props.audio_bitrate().unwrap_or(0)),
        sample_rate: Some(props.sample_rate().unwrap_or(0)),
        channels: Some(props.channels().unwrap_or(0)),
        duration_secs,
        format: Some(format!("{:?}", tagged.file_type())),
        tag_types,
        has_cover,
        extra,
    })
}

/// Parallel batch read tags (fast mode — tags only, no audio properties).
/// Uses rayon to read multiple files concurrently across CPU cores.
pub fn batch_read_tags(paths: &[(String, PathBuf)]) -> HashMap<String, TagData> {
    paths
        .par_iter()
        .filter_map(
            |(rel_path, canonical_path)| match read_tags_fast(canonical_path) {
                Ok(tags) => Some((rel_path.clone(), tags)),
                Err(e) => {
                    tracing::warn!("Failed to read tags for {}: {}", rel_path, e);
                    None
                }
            },
        )
        .collect()
}

/// Parallel batch read tags with full audio properties.
pub fn batch_read_tags_full(paths: &[(String, PathBuf)]) -> HashMap<String, TagData> {
    paths
        .par_iter()
        .filter_map(
            |(rel_path, canonical_path)| match read_tags_full(canonical_path) {
                Ok(tags) => Some((rel_path.clone(), tags)),
                Err(e) => {
                    tracing::warn!("Failed to read tags for {}: {}", rel_path, e);
                    None
                }
            },
        )
        .collect()
}

/// Write tag changes to a single audio file.
///
/// Crash-safe: the mutation runs against a temp copy which is atomically
/// renamed over the original, so a crash mid-write cannot truncate the file.
pub fn write_tags(path: &Path, changes: &TagWriteChanges) -> Result<(), TunewrightError> {
    let _lock = crate::locks::lock_file(path);
    crate::fsutil::atomic_file_update(path, |tmp| apply_tag_changes(tmp, changes))
}

pub fn modify_tags<F>(path: &Path, modify: F) -> Result<(), TunewrightError>
where
    F: FnOnce(&mut TagData),
{
    let _lock = crate::locks::lock_file(path);
    let original = read_tags_fast_with(path, true)?;
    let mut modified = original.clone();
    modify(&mut modified);
    let changes = TagWriteChanges::diff(&original, &modified);
    if changes == TagWriteChanges::default() {
        return Ok(());
    }
    crate::fsutil::atomic_file_update(path, |tmp| apply_tag_changes(tmp, &changes))
}

fn write_error(path: &Path, e: impl std::fmt::Display) -> TunewrightError {
    TunewrightError::TagWriteError(format!("{}: {}", path.display(), e))
}

type ExtraChanges = Vec<(ItemKey, Option<String>)>;

fn apply_tag_changes(path: &Path, changes: &TagWriteChanges) -> Result<(), TunewrightError> {
    if let Some(Some(year)) = changes.year.filter(|y| y.is_some_and(|y| y > 9999)) {
        return Err(write_error(
            path,
            format!("year {year} is out of range (0-9999)"),
        ));
    }
    // Keep cover art (default) so existing pictures survive the save, but
    // skip audio properties — they aren't needed for tag writes.
    let mut tagged = probe(path, ParseOptions::new().read_properties(false))
        .map_err(|e| write_error(path, e))?
        .read()
        .map_err(|e| write_error(path, e))?;

    let primary_type = tagged
        .primary_tag()
        .map(|t| t.tag_type())
        .unwrap_or_else(|| tagged.primary_tag_type());
    let options = write_options(path, &tagged);

    let file_custom = changes
        .extra
        .as_ref()
        .map(|_| primary_custom(path, &tagged))
        .unwrap_or_default();
    let custom_key = |key: &str| {
        let mut keys = file_custom.iter().map(|(k, _)| k.as_str());
        keys.clone()
            .find(|k| *k == key)
            .or_else(|| keys.find(|k| k.eq_ignore_ascii_case(key)))
    };
    let mut extra = ExtraChanges::new();
    let mut primary_extra = ExtraChanges::new();
    let mut custom = Vec::new();
    for (key, value) in changes.extra.iter().flatten() {
        let item_key = string_to_item_key(key).filter(|k| !is_standard_key(*k));
        extra.extend(item_key.map(|k| (k, value.clone())));
        let file_key = custom_key(key);
        match item_key.filter(|k| {
            k.map_key(primary_type).is_some()
                && file_key.is_none_or(|c| c != key && item_key_to_string(*k) == *key)
        }) {
            Some(k) => primary_extra.push((k, value.clone())),
            None => custom.push((file_key.unwrap_or(key), value.as_deref())),
        }
    }

    let secondary_types: Vec<TagType> = tagged
        .tags()
        .iter()
        .map(|t| t.tag_type())
        .filter(|&t| t != primary_type)
        .collect();
    let mut concrete_types = Vec::new();
    for t_type in secondary_types {
        let unchanged = match tagged.tag_mut(t_type) {
            Some(_) if matches!(t_type, TagType::Ape | TagType::RiffInfo) => {
                concrete_types.push(t_type);
                true
            }
            Some(tag) => !apply_changes(tag, changes, &extra, false)?,
            None => false,
        };
        if unchanged {
            tagged.remove(t_type);
        }
    }

    update_primary(path, &mut tagged, primary_type, &custom, options, |tag| {
        apply_changes(tag, changes, &primary_extra, true).map(drop)
    })?;

    tagged
        .save_to_path(path, options)
        .map_err(|e| write_error(path, e))?;

    for t_type in concrete_types {
        match t_type {
            TagType::Ape => update_mpeg_ape(path, changes, &extra)?,
            _ => update_wav_riff_info(path, changes, &extra)?,
        }
    }
    Ok(())
}

pub(crate) fn update_primary(
    path: &Path,
    tagged: &mut TaggedFile,
    primary_type: TagType,
    custom: &[(&str, Option<&str>)],
    options: WriteOptions,
    edit: impl FnOnce(&mut Tag) -> Result<(), TunewrightError>,
) -> Result<(), TunewrightError> {
    let mut tag = tagged
        .remove(primary_type)
        .unwrap_or_else(|| Tag::new(primary_type));
    let unsupported = |key: &str| write_error(path, format!("unsupported tag field '{key}'"));
    let parse_options = ParseOptions::new().read_properties(false);
    let Some(mut native) = Native::load(path, tagged.file_type(), &tag, parse_options)
        .map_err(|e| write_error(path, e))?
    else {
        if let Some((key, _)) = custom.first() {
            return Err(unsupported(key));
        }
        edit(&mut tag)?;
        tagged.insert_tag(tag);
        return Ok(());
    };
    native.edit(edit)?;
    for &(key, value) in custom {
        if !native.set_custom(key, value) {
            return Err(unsupported(key));
        }
    }
    native.save(path, options).map_err(|e| write_error(path, e))
}

pub(crate) fn write_options(path: &Path, tagged: &TaggedFile) -> WriteOptions {
    let id3v23 = || {
        tagged.tag(TagType::Id3v2)?;
        let file = &mut std::fs::File::open(path).ok()?;
        let options = ParseOptions::new()
            .read_properties(false)
            .read_cover_art(false);
        let tag = match tagged.file_type() {
            FileType::Mpeg => MpegFile::read_from(file, options).ok()?.remove_id3v2(),
            FileType::Aac => AacFile::read_from(file, options).ok()?.remove_id3v2(),
            FileType::Aiff => AiffFile::read_from(file, options).ok()?.remove_id3v2(),
            FileType::Wav => WavFile::read_from(file, options).ok()?.remove_id3v2(),
            _ => None,
        };
        Some(tag?.original_version() == Id3v2Version::V3)
    };
    WriteOptions::default().use_id3v23(id3v23().unwrap_or(false))
}

const ITUNES_MEAN: &str = "com.apple.iTunes";

enum Native {
    Vorbis(VorbisComments),
    Ape(ApeTag),
    Id3v2(Id3v2Tag),
    Ilst(Ilst),
}

impl Native {
    fn load(
        path: &Path,
        file_type: FileType,
        primary: &Tag,
        options: ParseOptions,
    ) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let open = || std::fs::File::open(path);
        Ok(Some(match file_type {
            FileType::Flac => {
                let mut flac = FlacFile::read_from(&mut open()?, options)?;
                let mut comments = flac.remove_vorbis_comments().unwrap_or_default();
                for (picture, info) in flac.remove_pictures() {
                    comments.insert_picture(picture, Some(info))?;
                }
                Self::Vorbis(comments)
            }
            FileType::Opus => Self::Vorbis(
                OpusFile::read_from(&mut open()?, options)?
                    .vorbis_comments()
                    .clone(),
            ),
            FileType::Vorbis => Self::Vorbis(
                VorbisFile::read_from(&mut open()?, options)?
                    .vorbis_comments()
                    .clone(),
            ),
            FileType::Speex => Self::Vorbis(
                SpeexFile::read_from(&mut open()?, options)?
                    .vorbis_comments()
                    .clone(),
            ),
            FileType::Ape => Self::Ape(
                ApeFile::read_from(&mut open()?, options)?
                    .remove_ape()
                    .unwrap_or_default(),
            ),
            FileType::WavPack => Self::Ape(
                WavPackFile::read_from(&mut open()?, options)?
                    .remove_ape()
                    .unwrap_or_default(),
            ),
            FileType::Mpc => Self::Ape(
                MpcFile::read_from(&mut open()?, options)?
                    .remove_ape()
                    .unwrap_or_default(),
            ),
            _ => match primary.tag_type() {
                TagType::Id3v2 => Self::Id3v2(primary.clone().into()),
                TagType::Mp4Ilst => Self::Ilst(primary.clone().into()),
                _ => return Ok(None),
            },
        }))
    }

    fn edit<R>(&mut self, edit: impl FnOnce(&mut Tag) -> R) -> R {
        match self {
            Self::Vorbis(t) => {
                let vendor = t.vendor().to_string();
                let mut encoders: Vec<String> = t.get_all("ENCODER").map(str::to_string).collect();
                let result = split_edit(t, |tag| {
                    let before: Vec<String> = tag
                        .get_strings(ItemKey::EncoderSoftware)
                        .map(str::to_string)
                        .collect();
                    let result = edit(tag);
                    let after: Vec<String> = tag
                        .get_strings(ItemKey::EncoderSoftware)
                        .map(str::to_string)
                        .collect();
                    if before != after {
                        encoders = after;
                    }
                    tag.remove_key(ItemKey::EncoderSoftware);
                    result
                });
                t.set_vendor(vendor);
                remove_vorbis(t, "ENCODER");
                for encoder in encoders {
                    t.push("ENCODER".to_string(), encoder);
                }
                result
            }
            Self::Ape(t) => {
                let pictures = ape_pictures(t);
                for key in lofty::ape::APE_PICTURE_TYPES {
                    t.remove(key);
                }
                split_edit(t, |tag| {
                    for picture in pictures {
                        tag.push_picture(picture);
                    }
                    edit(tag)
                })
            }
            Self::Id3v2(t) => split_edit(t, edit),
            Self::Ilst(t) => split_edit(t, edit),
        }
    }

    fn custom(&self) -> Vec<(String, String)> {
        let pair = |k: &str, v: &str| (k.to_string(), v.to_string());
        match self {
            Self::Vorbis(t) => t
                .clone()
                .split_tag()
                .0
                .items()
                .map(|(k, v)| pair(k, v))
                .collect(),
            Self::Ape(t) => ApeTag::from(t.clone().split_tag().0)
                .into_iter()
                .filter_map(|i| Some(pair(i.key(), i.value().text()?)))
                .collect(),
            Self::Id3v2(t) => t
                .clone()
                .split_tag()
                .0
                .iter()
                .filter_map(|f| match f {
                    Frame::UserText(ExtendedTextFrame {
                        description,
                        content,
                        ..
                    }) => Some(pair(description, content)),
                    _ => None,
                })
                .collect(),
            Self::Ilst(t) => Ilst::from(t.clone().split_tag().0)
                .into_iter()
                .filter_map(|a| match (a.ident(), a.data().next()) {
                    (AtomIdent::Freeform { mean, name }, Some(AtomData::UTF8(v)))
                        if mean == ITUNES_MEAN =>
                    {
                        Some(pair(name, v))
                    }
                    _ => None,
                })
                .collect(),
        }
    }

    fn set_custom(&mut self, key: &str, value: Option<&str>) -> bool {
        let value = value.filter(|v| !v.is_empty());
        match self {
            Self::Vorbis(t) => {
                match value {
                    Some(v) => t.insert(key.to_string(), v.to_string()),
                    None => remove_vorbis(t, key),
                }
                value.is_none() || t.get(key).is_some()
            }
            Self::Ape(t) => match value {
                Some(v) => ApeItem::new(key.to_string(), ItemValue::Text(v.to_string()))
                    .map(|item| t.insert(item))
                    .is_ok(),
                None => {
                    t.remove(key);
                    true
                }
            },
            Self::Id3v2(t) => {
                match value {
                    Some(v) => t.insert_user_text(key.to_string(), v.to_string()),
                    None => t.remove_user_text(key),
                };
                true
            }
            Self::Ilst(t) => {
                let ident = AtomIdent::Freeform {
                    mean: ITUNES_MEAN.into(),
                    name: key.to_string().into(),
                };
                t.retain(|a| a.ident() != &ident);
                if let Some(v) = value {
                    t.insert(Atom::new(ident, AtomData::UTF8(v.to_string())));
                }
                true
            }
        }
    }

    fn save(
        &self,
        path: &Path,
        options: WriteOptions,
    ) -> Result<(), lofty::error::FileEncodingError> {
        match self {
            Self::Vorbis(t) => t.save_to_path(path, options),
            Self::Ape(t) => t.save_to_path(path, options),
            Self::Id3v2(t) => {
                let mut options = options;
                if t.iter().any(|f| ID3V24_ONLY_FRAMES.contains(&f.id_str())) {
                    options.use_id3v23(false);
                }
                t.save_to_path(path, options)
            }
            Self::Ilst(t) => t.save_to_path(path, options),
        }
    }
}

const ID3V24_ONLY_FRAMES: &[&str] = &[
    "ASPI", "EQU2", "RVA2", "SEEK", "SIGN", "TDEN", "TDRL", "TDTG", "TMOO", "TPRO", "TSOA", "TSOP",
    "TSOT", "TSST",
];

fn ape_pictures(t: &ApeTag) -> Vec<lofty::picture::Picture> {
    lofty::ape::APE_PICTURE_TYPES
        .iter()
        .filter_map(|key| match t.get(key)?.value() {
            ItemValue::Binary(bytes) => lofty::picture::Picture::from_ape_bytes(key, bytes).ok(),
            _ => None,
        })
        .collect()
}

pub(crate) fn ordered_tags(tagged: &TaggedFile) -> Vec<&Tag> {
    let primary = tagged.primary_tag_type();
    let file_type = tagged.file_type();
    let mut tags: Vec<&Tag> = tagged
        .tags()
        .iter()
        .filter(|t| file_type.tag_support(t.tag_type()).is_writable())
        .collect();
    tags.sort_by_key(|t| (t.tag_type() != primary, t.tag_type() == TagType::Id3v1));
    tags
}

pub(crate) fn primary_ape_pictures(
    path: &Path,
    tagged: &TaggedFile,
) -> Vec<lofty::picture::Picture> {
    let Some(primary) = tagged
        .primary_tag()
        .filter(|t| t.tag_type() == TagType::Ape)
    else {
        return Vec::new();
    };
    match Native::load(
        path,
        tagged.file_type(),
        primary,
        ParseOptions::new().read_properties(false),
    ) {
        Ok(Some(Native::Ape(t))) => ape_pictures(&t),
        _ => Vec::new(),
    }
}

fn remove_vorbis(t: &mut VorbisComments, key: &str) {
    let items: Vec<_> = t
        .take_items()
        .filter(|(k, _)| !k.eq_ignore_ascii_case(key))
        .collect();
    for (k, v) in items {
        t.push(k, v);
    }
}

fn split_edit<T, R>(native: &mut T, edit: impl FnOnce(&mut Tag) -> R) -> R
where
    T: SplitTag + Default,
    T::Remainder: MergeTag<Merged = T>,
{
    let (remainder, mut tag) = std::mem::take(native).split_tag();
    let result = edit(&mut tag);
    *native = remainder.merge_tag(tag);
    result
}

fn primary_custom(path: &Path, tagged: &TaggedFile) -> Vec<(String, String)> {
    tagged
        .primary_tag()
        .and_then(|t| Native::load(path, tagged.file_type(), t, fast_parse_options()).ok()?)
        .map(|native| native.custom())
        .unwrap_or_default()
}

fn update_mpeg_ape(
    path: &Path,
    changes: &TagWriteChanges,
    extra: &ExtraChanges,
) -> Result<(), TunewrightError> {
    let mut file = MpegFile::read_from(
        &mut std::fs::File::open(path)?,
        ParseOptions::new().read_properties(false),
    )
    .map_err(|e| write_error(path, e))?;
    let Some(ape) = file.remove_ape() else {
        return Ok(());
    };
    let (remainder, mut tag) = ape.split_tag();
    if apply_changes(&mut tag, changes, extra, false)? {
        remainder
            .merge_tag(tag)
            .save_to_path(path, WriteOptions::default())
            .map_err(|e| write_error(path, e))?;
    }
    Ok(())
}

pub(crate) fn remove_mpeg_ape_pictures(path: &Path, keys: &[&str]) -> Result<(), TunewrightError> {
    let mut file = MpegFile::read_from(
        &mut std::fs::File::open(path)?,
        ParseOptions::new().read_properties(false),
    )
    .map_err(|e| write_error(path, e))?;
    let Some(mut ape) = file.remove_ape() else {
        return Ok(());
    };
    let keys: Vec<&str> = keys
        .iter()
        .copied()
        .filter(|key| ape.get(key).is_some())
        .collect();
    if keys.is_empty() {
        return Ok(());
    }
    for key in keys {
        ape.remove(key);
    }
    ape.save_to_path(path, WriteOptions::default())
        .map_err(|e| write_error(path, e))
}

fn update_wav_riff_info(
    path: &Path,
    changes: &TagWriteChanges,
    extra: &ExtraChanges,
) -> Result<(), TunewrightError> {
    let mut file = WavFile::read_from(
        &mut std::fs::File::open(path)?,
        ParseOptions::new().read_properties(false),
    )
    .map_err(|e| write_error(path, e))?;
    let Some(info) = file.remove_riff_info() else {
        return Ok(());
    };
    let mut tag = Tag::from(info.clone());
    if !apply_changes(&mut tag, changes, extra, false)? {
        return Ok(());
    }
    let mut updated = RiffInfoList::from(tag);
    for (key, value) in &info {
        if ItemKey::from_key(TagType::RiffInfo, key).is_none() {
            updated.insert(key.clone(), value.clone());
        }
    }
    updated
        .save_to_path(path, WriteOptions::default())
        .map_err(|e| write_error(path, e))
}

fn apply_changes(
    tag: &mut Tag,
    changes: &TagWriteChanges,
    extra: &ExtraChanges,
    add: bool,
) -> Result<bool, TunewrightError> {
    let before: Vec<TagItem> = tag.items().cloned().collect();
    let num = |v: Option<Option<u32>>| v.map(|n| n.map(|n| n.to_string()));
    let fields = [
        (ItemKey::TrackTitle, changes.title.clone()),
        (ItemKey::TrackArtist, changes.artist.clone()),
        (ItemKey::AlbumTitle, changes.album.clone()),
        (ItemKey::AlbumArtist, changes.album_artist.clone()),
        (ItemKey::Genre, changes.genre.clone()),
        (ItemKey::Comment, changes.comment.clone()),
        (ItemKey::Composer, changes.composer.clone()),
        (ItemKey::TrackNumber, num(changes.track_number)),
        (ItemKey::TrackTotal, num(changes.track_total)),
        (ItemKey::DiscNumber, num(changes.disc_number)),
        (ItemKey::DiscTotal, num(changes.disc_total)),
    ];
    let mut rejected = Vec::new();
    let extra = extra.iter().map(|(k, v)| (*k, Some(v.clone())));
    for (key, change) in fields.into_iter().chain(extra) {
        if !set_item(tag, key, change, add) {
            rejected.push(key);
        }
    }

    if let Some(year) = changes.year {
        let existing = tag
            .get_string(ItemKey::RecordingDate)
            .or_else(|| tag.get_string(ItemKey::Year))
            .map(str::to_string);
        // RecordingDate is the cross-format date key; ItemKey::Year isn't mapped for ID3v2.
        tag.remove_key(ItemKey::Year);
        tag.remove_key(ItemKey::RecordingDate);
        if let Some(year) = year.filter(|_| add || existing.is_some()) {
            let date = existing
                .filter(|d| parse_year(d) == Some(year))
                .unwrap_or_else(|| year.to_string());
            tag.push(TagItem::new(ItemKey::RecordingDate, ItemValue::Text(date)));
        }
    }

    if tag.tag_type() == TagType::Id3v2 {
        if tag.track().is_none() {
            tag.remove_key(ItemKey::TrackTotal);
        }
        if tag.disk().is_none() {
            tag.remove_key(ItemKey::DiscTotal);
        }
    }

    if add && !rejected.is_empty() {
        return Err(TunewrightError::TagWriteError(format!(
            "{:?} tags do not support {:?}",
            tag.tag_type(),
            rejected
        )));
    }
    Ok(!tag.items().eq(before.iter()))
}

fn set_item(tag: &mut Tag, key: ItemKey, change: Option<Option<String>>, add: bool) -> bool {
    let Some(value) = change else {
        return true;
    };
    let existed = tag.get(key).is_some();
    tag.remove_key(key);
    match value.filter(|v| !v.is_empty()) {
        Some(v) if add || existed => tag.push(TagItem::new(key, ItemValue::Text(v))),
        _ => true,
    }
}

/// Parallel batch write tags; per-path locks serialize conflicting writes.
pub fn batch_write_tags(changes: &[(String, PathBuf, TagWriteChanges)]) -> Vec<WriteResult> {
    changes
        .par_iter()
        .map(
            |(id, canonical_path, ch)| match write_tags(canonical_path, ch) {
                Ok(()) => WriteResult {
                    id: id.clone(),
                    status: "ok".to_string(),
                    error: None,
                },
                Err(e) => {
                    tracing::error!("Tag write failed for {}: {e}", canonical_path.display());
                    WriteResult {
                        id: id.clone(),
                        status: "error".to_string(),
                        error: Some(
                            match e {
                                TunewrightError::TagReadError(_) => "Failed to read tags",
                                _ => "Failed to write tags",
                            }
                            .to_string(),
                        ),
                    }
                }
            },
        )
        .collect()
}

fn first_string<F>(tags: &[&Tag], accessor: F) -> Option<String>
where
    F: Fn(&Tag) -> Option<std::borrow::Cow<'_, str>>,
{
    tags.iter()
        .find_map(|t| accessor(t).map(|s| s.to_string()))
        .filter(|s| !s.is_empty())
}

fn first_item_value(tags: &[&Tag], key: ItemKey) -> Option<String> {
    tags.iter()
        .find_map(|t| t.get_string(key).filter(|s| !s.is_empty()))
        .map(|s| s.to_string())
}

/// Read the year, preferring `RecordingDate` (cross-format) then `Year`.
fn read_year(tag: &Tag) -> Option<u32> {
    tag.get_string(ItemKey::RecordingDate)
        .or_else(|| tag.get_string(ItemKey::Year))
        .and_then(parse_year)
}

/// Parse the leading year from a date string, e.g. "2021-05-30" -> 2021.
fn parse_year(s: &str) -> Option<u32> {
    let digits: String = s
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .take(4)
        .collect();
    digits.parse::<u32>().ok().filter(|&y| y > 0)
}

/// Fields already handled by the standard TagData fields
fn is_standard_key(key: ItemKey) -> bool {
    matches!(
        key,
        ItemKey::TrackTitle
            | ItemKey::TrackArtist
            | ItemKey::AlbumTitle
            | ItemKey::AlbumArtist
            | ItemKey::TrackNumber
            | ItemKey::TrackTotal
            | ItemKey::DiscNumber
            | ItemKey::DiscTotal
            | ItemKey::Genre
            | ItemKey::Comment
            | ItemKey::Year
            | ItemKey::RecordingDate
            | ItemKey::Composer
    )
}

/// Convert an ItemKey to a canonical string key for the extra tags map
fn item_key_to_string(key: ItemKey) -> String {
    format!("{:?}", key)
}

/// Convert a string key back to an ItemKey for writing (case-insensitive)
fn string_to_item_key(key: &str) -> Option<ItemKey> {
    ItemKey::VARIANTS
        .iter()
        .copied()
        .find(|k| item_key_to_string(*k).eq_ignore_ascii_case(key))
}

const MAX_EXTRA_TAGS: usize = 256;
const MAX_TAG_VALUE_BYTES: usize = 64 * 1024;

/// Collect all non-standard tag items into a HashMap.
/// Lyric keys (often multiple KB per file) are only kept when `include_lyrics`
/// is set; the fast batch path skips them. `capped` limits the item count and
/// value size for display reads.
fn collect_extra_tags(
    tags: &[&Tag],
    include_lyrics: bool,
    capped: bool,
    custom: Vec<(String, String)>,
) -> HashMap<String, String> {
    let (max_tags, max_value) = if capped {
        (MAX_EXTRA_TAGS, MAX_TAG_VALUE_BYTES)
    } else {
        (usize::MAX, usize::MAX)
    };
    let mut extra = HashMap::new();
    for tag in tags {
        for item in tag.items() {
            if is_standard_key(item.key()) {
                continue;
            }
            if !include_lyrics && matches!(item.key(), ItemKey::Lyrics | ItemKey::UnsyncLyrics) {
                continue;
            }
            if extra.len() >= max_tags {
                return extra;
            }
            let key = item_key_to_string(item.key());
            if extra.contains_key(&key) {
                continue; // first tag wins
            }
            if let ItemValue::Text(val) = item.value() {
                if !val.is_empty() && val.len() <= max_value {
                    extra.insert(key, val.to_string());
                }
            }
        }
    }
    for (key, val) in custom {
        if extra.len() >= max_tags {
            break;
        }
        if !val.is_empty() && val.len() <= max_value {
            extra.entry(key).or_insert(val);
        }
    }
    extra
}

#[cfg(test)]
mod tests {
    use super::{parse_year, read_year};
    use lofty::file::{AudioFile, TaggedFileExt};
    use lofty::tag::{Accessor, ItemKey, ItemValue, Tag, TagItem, TagType};

    #[test]
    fn parse_year_extracts_leading_year() {
        assert_eq!(parse_year("2021"), Some(2021));
        assert_eq!(parse_year("2021-05-30"), Some(2021));
        assert_eq!(parse_year("2021.05.30"), Some(2021));
        assert_eq!(parse_year(" 1998 "), Some(1998));
        assert_eq!(parse_year(""), None);
        assert_eq!(parse_year("n/a"), None);
        assert_eq!(parse_year("0"), None);
    }

    #[test]
    fn read_year_prefers_recording_date() {
        let mut tag = Tag::new(TagType::Id3v2);
        tag.push(TagItem::new(
            ItemKey::RecordingDate,
            ItemValue::Text("2019-01-02".into()),
        ));
        assert_eq!(read_year(&tag), Some(2019));
    }

    #[test]
    fn read_year_falls_back_to_year_key() {
        let mut tag = Tag::new(TagType::VorbisComments);
        tag.push(TagItem::new(ItemKey::Year, ItemValue::Text("2005".into())));
        assert_eq!(read_year(&tag), Some(2005));
    }

    #[test]
    fn test_write_tags_keeps_and_updates_secondary_tags() {
        use crate::types::TagWriteChanges;
        use lofty::config::WriteOptions;
        use lofty::probe::Probe;
        use lofty::tag::{ItemKey, ItemValue, Tag, TagItem, TagType};
        use std::fs::File;
        use std::io::Write;

        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("tunewright_audio_test_{}", nanos));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let file_path = temp_dir.join("test.wav");

        // Minimal WAV file bytes (RIFF, WAVE, fmt , data chunks)
        let wav_bytes = b"RIFF\x28\x00\x00\x00WAVEfmt \x10\x00\x00\x00\x01\x00\x01\x00\x44\xac\x00\x00\x88\x58\x01\x00\x02\x00\x10\x00data\x04\x00\x00\x00\x00\x00\x00\x00";
        let mut f = File::create(&file_path).unwrap();
        f.write_all(wav_bytes).unwrap();
        drop(f);

        // 1. Manually insert both RiffInfo (primary for WAV in some contexts) and ID3v2 tags
        let mut tagged = Probe::open(&file_path).unwrap().read().unwrap();

        let primary_type = tagged.primary_tag_type();
        let secondary_type = if primary_type == TagType::RiffInfo {
            TagType::Id3v2
        } else {
            TagType::RiffInfo
        };

        let mut primary_tag = Tag::new(primary_type);
        primary_tag.push(TagItem::new(
            ItemKey::TrackTitle,
            ItemValue::Text("Primary Title".to_string()),
        ));
        primary_tag.push(TagItem::new(
            ItemKey::TrackArtist,
            ItemValue::Text("Primary Artist".to_string()),
        ));
        tagged.insert_tag(primary_tag);

        let mut secondary_tag = Tag::new(secondary_type);
        secondary_tag.push(TagItem::new(
            ItemKey::TrackTitle,
            ItemValue::Text("Secondary Title".to_string()),
        ));
        secondary_tag.push(TagItem::new(
            ItemKey::Composer,
            ItemValue::Text("Secondary Composer".to_string()),
        ));
        tagged.insert_tag(secondary_tag);

        tagged
            .save_to_path(&file_path, WriteOptions::default())
            .unwrap();

        assert_eq!(secondary_type, TagType::RiffInfo);
        let open_wav = || {
            super::WavFile::read_from(
                &mut File::open(&file_path).unwrap(),
                lofty::config::ParseOptions::new(),
            )
            .unwrap()
        };
        let mut wav = open_wav();
        wav.riff_info_mut()
            .unwrap()
            .insert("IXYZ".to_string(), "keep me".to_string());
        wav.save_to_path(&file_path, WriteOptions::default())
            .unwrap();

        // 2. Write changes; the secondary tag must survive and track them
        let changes = TagWriteChanges {
            title: Some(Some("New Title".to_string())),
            artist: Some(Some("New Artist".to_string())),
            composer: Some(None),
            ..Default::default()
        };
        super::write_tags(&file_path, &changes).unwrap();

        // 3. Verify the result
        let tagged_after = Probe::open(&file_path).unwrap().read().unwrap();
        let primary_after = tagged_after.tag(primary_type).unwrap();
        assert_eq!(
            primary_after.get_string(ItemKey::TrackTitle),
            Some("New Title")
        );
        assert_eq!(
            primary_after.get_string(ItemKey::TrackArtist),
            Some("New Artist")
        );

        let secondary_after = tagged_after.tag(secondary_type).unwrap();
        assert_eq!(
            secondary_after.get_string(ItemKey::TrackTitle),
            Some("New Title")
        );
        assert_eq!(secondary_after.get_string(ItemKey::TrackArtist), None);
        assert_eq!(secondary_after.get_string(ItemKey::Composer), None);

        assert_eq!(open_wav().riff_info().unwrap().get("IXYZ"), Some("keep me"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_write_tags_id3v2_total_only_no_fabrication() {
        use crate::types::TagWriteChanges;
        use lofty::config::WriteOptions;
        use lofty::file::AudioFile;
        use lofty::probe::Probe;
        use lofty::tag::{Tag, TagType};
        use std::fs::File;
        use std::io::Write;

        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("tunewright_audio_test_{}", nanos));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let file_path = temp_dir.join("test.wav");

        // Minimal valid WAV (RIFF/WAVE/fmt/data)
        let wav_bytes = b"RIFF\x28\x00\x00\x00WAVEfmt \x10\x00\x00\x00\x01\x00\x01\x00\x44\xac\x00\x00\x88\x58\x01\x00\x02\x00\x10\x00data\x04\x00\x00\x00\x00\x00\x00\x00";
        let mut f = File::create(&file_path).unwrap();
        f.write_all(wav_bytes).unwrap();
        drop(f);

        // Manually insert an ID3v2 tag into the WAV file so primary_type == Id3v2
        {
            let mut tagged = Probe::open(&file_path).unwrap().read().unwrap();
            tagged.insert_tag(Tag::new(TagType::Id3v2));
            tagged
                .save_to_path(&file_path, WriteOptions::default())
                .unwrap();
        }

        // 1. Write only track_total and disc_total (no track_number or disc_number)
        let changes = TagWriteChanges {
            track_total: Some(Some(12)),
            disc_total: Some(Some(2)),
            ..Default::default()
        };
        super::write_tags(&file_path, &changes).unwrap();

        // 2. Read back and verify — no "0/N" fabrication
        let tagged_after = Probe::open(&file_path).unwrap().read().unwrap();
        if let Some(tag) = tagged_after.tag(TagType::Id3v2) {
            // Assert track number is NOT fabricated to 0
            assert_eq!(tag.track(), None, "track number must not be fabricated");
            assert_eq!(tag.disk(), None, "disc number must not be fabricated");
            // Since no track_number was present, track_total should also be absent in ID3v2
            assert_eq!(
                tag.track_total(),
                None,
                "track_total must not be written without track_number in ID3v2"
            );
            assert_eq!(
                tag.disk_total(),
                None,
                "disc_total must not be written without disc_number in ID3v2"
            );
        }

        // 3. Now write both track number AND track total together
        let changes2 = TagWriteChanges {
            track_number: Some(Some(3)),
            track_total: Some(Some(12)),
            disc_number: Some(Some(1)),
            disc_total: Some(Some(2)),
            ..Default::default()
        };
        super::write_tags(&file_path, &changes2).unwrap();

        // Verify it successfully writes both, no fabricated 0
        let tagged_after2 = Probe::open(&file_path).unwrap().read().unwrap();
        if let Some(tag2) = tagged_after2.tag(TagType::Id3v2) {
            assert_eq!(tag2.track(), Some(3));
            assert_eq!(tag2.track_total(), Some(12));
            assert_eq!(tag2.disk(), Some(1));
            assert_eq!(tag2.disk_total(), Some(2));
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_string_to_item_key_is_symmetric() {
        use lofty::tag::ItemKey;

        let keys = vec![
            ItemKey::AlbumTitle,
            ItemKey::SetSubtitle,
            ItemKey::ShowName,
            ItemKey::ContentGroup,
            ItemKey::TrackTitle,
            ItemKey::TrackSubtitle,
            ItemKey::OriginalAlbumTitle,
            ItemKey::OriginalArtist,
            ItemKey::OriginalLyricist,
            ItemKey::AlbumTitleSortOrder,
            ItemKey::AlbumArtistSortOrder,
            ItemKey::TrackTitleSortOrder,
            ItemKey::TrackArtistSortOrder,
            ItemKey::ShowNameSortOrder,
            ItemKey::ComposerSortOrder,
            ItemKey::AlbumArtist,
            ItemKey::AlbumArtists,
            ItemKey::TrackArtist,
            ItemKey::TrackArtists,
            ItemKey::Arranger,
            ItemKey::Writer,
            ItemKey::Composer,
            ItemKey::Conductor,
            ItemKey::Director,
            ItemKey::Engineer,
            ItemKey::Lyricist,
            ItemKey::MixDj,
            ItemKey::MixEngineer,
            ItemKey::Performer,
            ItemKey::Producer,
            ItemKey::Publisher,
            ItemKey::Label,
            ItemKey::InternetRadioStationName,
            ItemKey::InternetRadioStationOwner,
            ItemKey::Remixer,
            ItemKey::DiscNumber,
            ItemKey::DiscTotal,
            ItemKey::TrackNumber,
            ItemKey::TrackTotal,
            ItemKey::Popularimeter,
            ItemKey::ParentalAdvisory,
            ItemKey::RecordingDate,
            ItemKey::Year,
            ItemKey::ReleaseDate,
            ItemKey::OriginalReleaseDate,
            ItemKey::Isrc,
            ItemKey::Barcode,
            ItemKey::AcoustId,
            ItemKey::AcoustIdFingerprint,
            ItemKey::CatalogNumber,
            ItemKey::Work,
            ItemKey::Movement,
            ItemKey::MovementNumber,
            ItemKey::MovementTotal,
            ItemKey::ReleaseCountry,
            ItemKey::MusicBrainzRecordingId,
            ItemKey::MusicBrainzTrackId,
            ItemKey::MusicBrainzReleaseId,
            ItemKey::MusicBrainzReleaseGroupId,
            ItemKey::MusicBrainzArtistId,
            ItemKey::MusicBrainzReleaseArtistId,
            ItemKey::MusicBrainzWorkId,
            ItemKey::MusicBrainzReleaseType,
            ItemKey::FlagCompilation,
            ItemKey::FlagPodcast,
            ItemKey::FileOwner,
            ItemKey::TaggingTime,
            ItemKey::Length,
            ItemKey::OriginalFileName,
            ItemKey::OriginalMediaType,
            ItemKey::EncodedBy,
            ItemKey::EncoderSoftware,
            ItemKey::EncoderSettings,
            ItemKey::EncodingTime,
            ItemKey::ReplayGainAlbumGain,
            ItemKey::ReplayGainAlbumPeak,
            ItemKey::ReplayGainTrackGain,
            ItemKey::ReplayGainTrackPeak,
            ItemKey::AudioFileUrl,
            ItemKey::AudioSourceUrl,
            ItemKey::CommercialInformationUrl,
            ItemKey::CopyrightUrl,
            ItemKey::TrackArtistUrl,
            ItemKey::RadioStationUrl,
            ItemKey::PaymentUrl,
            ItemKey::PublisherUrl,
            ItemKey::Genre,
            ItemKey::InitialKey,
            ItemKey::Color,
            ItemKey::Mood,
            ItemKey::Bpm,
            ItemKey::IntegerBpm,
            ItemKey::CopyrightMessage,
            ItemKey::License,
            ItemKey::PodcastDescription,
            ItemKey::PodcastSeriesCategory,
            ItemKey::PodcastUrl,
            ItemKey::PodcastGlobalUniqueId,
            ItemKey::PodcastKeywords,
            ItemKey::Comment,
            ItemKey::Description,
            ItemKey::Language,
            ItemKey::Script,
            ItemKey::Lyrics,
            ItemKey::UnsyncLyrics,
            ItemKey::AppleXid,
            ItemKey::AppleId3v2ContentGroup,
        ];

        for key in keys {
            let key_str = format!("{:?}", key);
            let parsed = super::string_to_item_key(&key_str);
            assert_eq!(
                parsed,
                Some(key),
                "Failed to parse debug string of {:?} back to ItemKey",
                key
            );
        }
    }

    fn temp_flac(items: &[(ItemKey, &str)]) -> (std::path::PathBuf, std::path::PathBuf) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "tunewright_audio_flac_{nanos}_{}",
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.flac");
        std::fs::write(&path, b"fLaC\x80\x00\x00\x22\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00").unwrap();

        let mut tagged = lofty::probe::Probe::open(&path).unwrap().read().unwrap();
        let mut tag = Tag::new(TagType::VorbisComments);
        for (key, value) in items {
            tag.push(TagItem::new(*key, ItemValue::Text(value.to_string())));
        }
        tagged.insert_tag(tag);
        tagged
            .save_to_path(&path, lofty::config::WriteOptions::default())
            .unwrap();
        (dir, path)
    }

    fn vorbis(path: &std::path::Path) -> Tag {
        lofty::probe::Probe::open(path)
            .unwrap()
            .read()
            .unwrap()
            .tag(TagType::VorbisComments)
            .unwrap()
            .clone()
    }

    #[test]
    fn modify_tags_leaves_unrelated_fields_alone() {
        let (dir, path) = temp_flac(&[
            (ItemKey::TrackTitle, " Song "),
            (ItemKey::TrackArtist, "A"),
            (ItemKey::TrackArtist, "B"),
            (ItemKey::RecordingDate, "2021-05-30"),
        ]);

        super::modify_tags(&path, |t| {
            t.title = t.title.as_deref().map(|s| s.trim().to_string());
        })
        .unwrap();

        let tag = vorbis(&path);
        assert_eq!(tag.get_string(ItemKey::TrackTitle), Some("Song"));
        assert_eq!(
            tag.get_strings(ItemKey::TrackArtist).collect::<Vec<_>>(),
            ["A", "B"]
        );
        assert_eq!(tag.get_string(ItemKey::RecordingDate), Some("2021-05-30"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_tags_removes_string_numeric_and_extra_fields() {
        let (dir, path) = temp_flac(&[
            (ItemKey::TrackTitle, "Song"),
            (ItemKey::AlbumTitle, "Keep"),
            (ItemKey::TrackNumber, "3"),
            (ItemKey::RecordingDate, "2021-05-30"),
            (ItemKey::Bpm, "120"),
        ]);

        let changes = crate::types::TagWriteChanges {
            title: Some(None),
            year: Some(None),
            track_number: Some(None),
            extra: Some([("BPM".to_string(), None)].into()),
            ..Default::default()
        };
        super::write_tags(&path, &changes).unwrap();

        let tags = super::read_tags_fast(&path).unwrap();
        assert_eq!(tags.title, None);
        assert_eq!(tags.year, None);
        assert_eq!(tags.track_number, None);
        assert!(tags.extra.is_empty());
        assert_eq!(tags.album.as_deref(), Some("Keep"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_tags_year_keeps_full_date_when_year_matches() {
        let (dir, path) = temp_flac(&[(ItemKey::RecordingDate, "2021-05-30")]);
        let year = |y| crate::types::TagWriteChanges {
            year: Some(Some(y)),
            ..Default::default()
        };

        super::write_tags(&path, &year(2021)).unwrap();
        assert_eq!(
            vorbis(&path).get_string(ItemKey::RecordingDate),
            Some("2021-05-30")
        );

        super::write_tags(&path, &year(2022)).unwrap();
        assert_eq!(
            vorbis(&path).get_string(ItemKey::RecordingDate),
            Some("2022")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_tags_extra_keys_are_case_insensitive_and_invalid_keys_fail() {
        let (dir, path) = temp_flac(&[]);
        let extra = |key: &str| crate::types::TagWriteChanges {
            extra: Some([(key.to_string(), Some("128".to_string()))].into()),
            ..Default::default()
        };

        super::write_tags(&path, &extra("bpm")).unwrap();
        assert_eq!(vorbis(&path).get_string(ItemKey::Bpm), Some("128"));

        assert!(super::write_tags(&path, &extra("NO=FIELD")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn temp_file(name: &str, bytes: &[u8]) -> (std::path::PathBuf, std::path::PathBuf) {
        let (dir, _) = temp_flac(&[]);
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        (dir, path)
    }

    fn title(value: &str) -> crate::types::TagWriteChanges {
        crate::types::TagWriteChanges {
            title: Some(Some(value.into())),
            ..Default::default()
        }
    }

    fn custom(value: Option<&str>) -> crate::types::TagWriteChanges {
        crate::types::TagWriteChanges {
            extra: Some([("MYKEY".to_string(), value.map(str::to_string))].into()),
            ..Default::default()
        }
    }

    fn mykey(path: &std::path::Path) -> Option<String> {
        super::read_tags_fast(path).unwrap().extra.remove("MYKEY")
    }

    #[test]
    fn write_tags_keeps_unmapped_vorbis_comments() {
        use lofty::ogg::tag::VorbisComments;
        use lofty::tag::TagExt;

        let (flac_dir, flac) = temp_flac(&[(ItemKey::TrackTitle, "Old")]);
        let path = &flac;
        {
            let mut comments = VorbisComments::default();
            comments.push("MYKEY".into(), "x".into());
            comments.push("MYKEY".into(), "y".into());
            comments.push("ENCODER".into(), "Lavf63".into());
            comments
                .save_to_path(path, lofty::config::WriteOptions::default())
                .unwrap();

            super::write_tags(path, &title("New")).unwrap();

            let tags = super::read_tags_fast(path).unwrap();
            assert_eq!(tags.title.as_deref(), Some("New"), "{}", path.display());
            assert_eq!(tags.extra.get("MYKEY").map(String::as_str), Some("x"));
            let tagged = lofty::probe::Probe::open(path).unwrap().read().unwrap();
            let native = super::Native::load(
                path,
                tagged.file_type(),
                &Tag::new(TagType::VorbisComments),
                lofty::config::ParseOptions::new(),
            );
            let Ok(Some(super::Native::Vorbis(comments))) = native else {
                panic!("{} has no vorbis comments", path.display());
            };
            assert_eq!(comments.get_all("MYKEY").collect::<Vec<_>>(), ["x", "y"]);
            assert_eq!(comments.get_all("ENCODER").collect::<Vec<_>>(), ["Lavf63"]);
        }
        let _ = std::fs::remove_dir_all(flac_dir);
    }

    #[test]
    fn write_tags_round_trips_custom_extra_keys() {
        let mut frame = vec![0xFF, 0xFB, 0x90, 0x00];
        frame.resize(417, 0);
        let (flac_dir, flac) = temp_flac(&[]);
        let (mp3_dir, mp3) = temp_file("t.mp3", &frame.repeat(4));
        for path in [&flac, &mp3] {
            super::write_tags(path, &custom(Some("v1"))).unwrap();
            assert_eq!(mykey(path).as_deref(), Some("v1"), "{}", path.display());
            super::modify_tags(path, |t| {
                t.extra.insert("MYKEY".into(), "v2".into());
            })
            .unwrap();
            assert_eq!(mykey(path).as_deref(), Some("v2"));
            super::write_tags(path, &custom(None)).unwrap();
            assert_eq!(mykey(path), None);
        }

        super::write_tags(&mp3, &custom(Some("txxx"))).unwrap();
        let mpeg = super::MpegFile::read_from(
            &mut std::fs::File::open(&mp3).unwrap(),
            lofty::config::ParseOptions::new(),
        )
        .unwrap();
        assert_eq!(mpeg.id3v2().unwrap().get_user_text("MYKEY"), Some("txxx"));
        for dir in [flac_dir, mp3_dir] {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn flac_cover_art_survives_tag_edits() {
        let (dir, path) = temp_flac(&[(ItemKey::TrackTitle, "Old")]);
        let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0];
        crate::picture::embed_cover_art(&path, &png).unwrap();
        super::write_tags(&path, &custom(Some("x"))).unwrap();
        super::write_tags(&path, &title("New")).unwrap();

        let cover = crate::picture::extract_cover_art(&path).unwrap().unwrap();
        assert_eq!(cover.0, png);
        let tags = super::read_tags_fast(&path).unwrap();
        assert_eq!(tags.title.as_deref(), Some("New"));
        assert_eq!(tags.extra.get("MYKEY").map(String::as_str), Some("x"));

        crate::picture::embed_cover_art(&path, &png).unwrap();
        crate::picture::remove_cover_art(&path).unwrap();
        assert!(crate::picture::extract_cover_art(&path).unwrap().is_none());
        assert_eq!(mykey(&path).as_deref(), Some("x"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn format_is_detected_from_content_not_extension() {
        for name in ["mislabeled.ogg", "mislabeled.mp3", "mislabeled.m4a"] {
            let (dir, flac) = temp_flac(&[(ItemKey::TrackTitle, "Old")]);
            let path = dir.join(name);
            std::fs::rename(&flac, &path).unwrap();

            assert_eq!(
                super::read_tags_fast(&path).unwrap().title.as_deref(),
                Some("Old"),
                "{name}"
            );
            super::write_tags(&path, &title("New")).unwrap();
            assert_eq!(
                super::read_tags_fast(&path).unwrap().title.as_deref(),
                Some("New"),
                "{name}"
            );
            assert!(std::fs::read(&path).unwrap().starts_with(b"fLaC"), "{name}");
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    fn pcm_chunk_file(name: &str, aiff: bool) -> (std::path::PathBuf, std::path::PathBuf, Vec<u8>) {
        let samples: Vec<u8> = (0..4000u32)
            .flat_map(|i| ((i * 37) as u16).to_le_bytes())
            .collect();
        let mut body = Vec::new();
        if aiff {
            let mut comm = Vec::new();
            comm.extend_from_slice(&1u16.to_be_bytes());
            comm.extend_from_slice(&((samples.len() / 2) as u32).to_be_bytes());
            comm.extend_from_slice(&16u16.to_be_bytes());
            comm.extend_from_slice(&[0x40, 0x0E, 0xAC, 0x44, 0, 0, 0, 0, 0, 0]);
            body.extend_from_slice(b"AIFF");
            body.extend_from_slice(b"COMM");
            body.extend_from_slice(&(comm.len() as u32).to_be_bytes());
            body.extend_from_slice(&comm);
            body.extend_from_slice(b"SSND");
            body.extend_from_slice(&((samples.len() + 8) as u32).to_be_bytes());
            body.extend_from_slice(&[0; 8]);
            body.extend_from_slice(&samples);
        } else {
            body.extend_from_slice(b"WAVEfmt ");
            body.extend_from_slice(&16u32.to_le_bytes());
            body.extend_from_slice(&[1, 0, 1, 0]);
            body.extend_from_slice(&44100u32.to_le_bytes());
            body.extend_from_slice(&88200u32.to_le_bytes());
            body.extend_from_slice(&[2, 0, 16, 0]);
            body.extend_from_slice(b"data");
            body.extend_from_slice(&(samples.len() as u32).to_le_bytes());
            body.extend_from_slice(&samples);
        }
        let mut file = Vec::new();
        file.extend_from_slice(if aiff { b"FORM" } else { b"RIFF" });
        let size = body.len() as u32;
        file.extend_from_slice(&if aiff {
            size.to_be_bytes()
        } else {
            size.to_le_bytes()
        });
        file.extend_from_slice(&body);
        let (dir, path) = temp_file(name, &file);
        (dir, path, samples)
    }

    fn chunk_layout_is_valid(bytes: &[u8], aiff: bool) -> bool {
        let read = |b: &[u8]| {
            let a: [u8; 4] = b.try_into().unwrap();
            (if aiff {
                u32::from_be_bytes(a)
            } else {
                u32::from_le_bytes(a)
            }) as usize
        };
        if read(&bytes[4..8]) + 8 != bytes.len() {
            return false;
        }
        let mut i = 12;
        while i + 8 <= bytes.len() {
            i += 8 + read(&bytes[i + 4..i + 8]);
            i += i & 1;
        }
        i == bytes.len()
    }

    #[test]
    fn growing_and_shrinking_tags_keep_wav_and_aiff_intact() {
        for (name, aiff) in [("t.wav", false), ("t.aiff", true)] {
            let (dir, path, samples) = pcm_chunk_file(name, aiff);
            super::write_tags(&path, &title("Short")).unwrap();
            let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
                .into_iter()
                .chain(std::iter::repeat_n(7u8, 5000))
                .collect::<Vec<_>>();
            crate::picture::embed_cover_art(&path, &png).unwrap();
            let bytes = std::fs::read(&path).unwrap();
            assert!(chunk_layout_is_valid(&bytes, aiff), "{name} after growing");
            crate::picture::remove_cover_art(&path).unwrap();
            super::write_tags(&path, &title("A much longer title than before")).unwrap();

            let bytes = std::fs::read(&path).unwrap();
            assert!(
                chunk_layout_is_valid(&bytes, aiff),
                "{name} after shrinking"
            );
            assert!(
                bytes.windows(samples.len()).any(|w| w == samples),
                "{name} audio changed"
            );
            let tags = super::read_tags_fast(&path).unwrap();
            assert_eq!(
                tags.title.as_deref(),
                Some("A much longer title than before")
            );
            assert!(crate::picture::extract_cover_art(&path).unwrap().is_none());
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn flac_primary_tag_wins_over_stale_id3v2() {
        use lofty::picture::{MimeType, Picture, PictureType};
        use lofty::tag::TagExt;

        let (dir, path) = temp_flac(&[(ItemKey::TrackTitle, "Correct")]);
        let mut id3 = Tag::new(TagType::Id3v2);
        id3.set_title("Stale".to_string());
        id3.set_artist("Real Artist".to_string());
        id3.push_picture(
            Picture::unchecked(vec![0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3, 4])
                .pic_type(PictureType::CoverFront)
                .mime_type(MimeType::Jpeg)
                .build(),
        );
        let mut bytes = Vec::new();
        id3.dump_to(&mut bytes, lofty::config::WriteOptions::default())
            .unwrap();
        bytes.extend(std::fs::read(&path).unwrap());
        std::fs::write(&path, bytes).unwrap();

        assert_eq!(
            super::read_tags_fast(&path).unwrap().title.as_deref(),
            Some("Correct")
        );
        let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 9, 9];
        crate::picture::embed_cover_art(&path, &png).unwrap();
        assert_eq!(
            crate::picture::extract_cover_art(&path).unwrap().unwrap().0,
            png
        );
        crate::picture::remove_cover_art(&path).unwrap();
        assert!(crate::picture::extract_cover_art(&path).unwrap().is_none());
        let tags = super::read_tags_fast(&path).unwrap();
        assert_eq!(tags.title.as_deref(), Some("Correct"));
        assert_eq!(tags.artist, None);
        assert!(std::fs::read(&path).unwrap().starts_with(b"ID3"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn clearing_a_field_ignores_read_only_id3v2_and_keeps_it_intact() {
        use lofty::picture::{MimeType, Picture, PictureType};
        use lofty::tag::TagExt;

        let (dir, path) = temp_flac(&[(ItemKey::Genre, "Jazz"), (ItemKey::Year, "2001")]);
        let mut id3 = Tag::new(TagType::Id3v2);
        id3.set_genre("Rock".to_string());
        id3.set_artist("Real Artist".to_string());
        id3.insert_text(ItemKey::RecordingDate, "1999".to_string());
        id3.push_picture(
            Picture::unchecked(vec![0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3, 4])
                .pic_type(PictureType::CoverFront)
                .mime_type(MimeType::Jpeg)
                .build(),
        );
        let mut bytes = Vec::new();
        id3.dump_to(&mut bytes, lofty::config::WriteOptions::default())
            .unwrap();
        bytes.extend(std::fs::read(&path).unwrap());
        std::fs::write(&path, bytes).unwrap();

        let changes = crate::types::TagWriteChanges {
            genre: Some(None),
            ..Default::default()
        };
        super::write_tags(&path, &changes).unwrap();
        let tags = super::read_tags_fast(&path).unwrap();
        assert_eq!(tags.genre, None);
        assert_eq!(tags.year, Some(2001));
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"ID3") && bytes.windows(4).any(|w| w == b"Rock"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn embedding_cover_replaces_untyped_existing_art() {
        use lofty::picture::{MimeType, Picture, PictureType};

        let (dir, path) = temp_flac(&[(ItemKey::TrackTitle, "Song")]);
        let old = [0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3, 4];
        let mut tagged = lofty::probe::Probe::open(&path).unwrap().read().unwrap();
        tagged.primary_tag_mut().unwrap().push_picture(
            Picture::unchecked(old.to_vec())
                .pic_type(PictureType::Other)
                .mime_type(MimeType::Jpeg)
                .build(),
        );
        tagged
            .save_to_path(&path, lofty::config::WriteOptions::default())
            .unwrap();

        let new = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 9, 9];
        crate::picture::embed_cover_art(&path, &new).unwrap();

        let tagged = lofty::probe::Probe::open(&path).unwrap().read().unwrap();
        let pictures: Vec<_> = tagged
            .tags()
            .iter()
            .flat_map(|t| t.pictures().iter())
            .collect();
        assert_eq!(pictures.len(), 1);
        assert_eq!(pictures[0].data(), new);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn write_tags_keeps_unmapped_ape_items_in_mp3() {
        use lofty::ape::{ApeItem, ApeTag};
        use lofty::config::{ParseOptions, WriteOptions};
        use lofty::tag::TagExt;

        let (dir, _) = temp_flac(&[]);
        let path = dir.join("test.mp3");
        let mut frame = vec![0xFF, 0xFB, 0x90, 0x00];
        frame.resize(417, 0);
        std::fs::write(&path, frame.repeat(4)).unwrap();

        let mut ape = ApeTag::new();
        ape.insert(ApeItem::new("Title".into(), ItemValue::Text("Old".into())).unwrap());
        ape.insert(
            ApeItem::new("MP3GAIN_UNDO".into(), ItemValue::Text("+001,+001,N".into())).unwrap(),
        );
        ape.save_to_path(&path, WriteOptions::default()).unwrap();

        let changes = crate::types::TagWriteChanges {
            title: Some(Some("New".into())),
            ..Default::default()
        };
        super::write_tags(&path, &changes).unwrap();

        let mpeg = super::MpegFile::read_from(
            &mut std::fs::File::open(&path).unwrap(),
            ParseOptions::new(),
        )
        .unwrap();
        let ape = mpeg.ape().unwrap();
        assert_eq!(ape.title().as_deref(), Some("New"));
        assert_eq!(
            ape.get("MP3GAIN_UNDO").and_then(|i| i.value().text()),
            Some("+001,+001,N")
        );
        assert_eq!(mpeg.id3v2().unwrap().title().as_deref(), Some("New"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn mp3_cover_changes_clear_secondary_ape_art() {
        use lofty::ape::{ApeItem, ApeTag};
        use lofty::config::{ParseOptions, WriteOptions};
        use lofty::tag::TagExt;

        let (dir, _) = temp_flac(&[]);
        let path = dir.join("test.mp3");
        let mut frame = vec![0xFF, 0xFB, 0x90, 0x00];
        frame.resize(417, 0);
        std::fs::write(&path, frame.repeat(4)).unwrap();
        let mut ape = ApeTag::new();
        ape.insert(ApeItem::new("Title".into(), ItemValue::Text("Song".into())).unwrap());
        ape.insert(
            ApeItem::new(
                "Cover Art (Front)".into(),
                ItemValue::Binary(b"c.jpg\0\xFF\xD8\xFF\xE0\x01\x02".to_vec()),
            )
            .unwrap(),
        );
        ape.insert(
            ApeItem::new(
                "Cover Art (Back)".into(),
                ItemValue::Binary(b"b.jpg\0\xFF\xD8\xFF\xE0\x03\x04".to_vec()),
            )
            .unwrap(),
        );
        ape.save_to_path(&path, WriteOptions::default()).unwrap();
        let ape_item = |path: &std::path::Path, key: &str| {
            super::MpegFile::read_from(&mut std::fs::File::open(path).unwrap(), ParseOptions::new())
                .unwrap()
                .ape()
                .and_then(|a| a.get(key).map(|_| ()))
                .is_some()
        };
        let ape_art = |path: &std::path::Path| ape_item(path, "Cover Art (Front)");

        let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 9, 9];
        crate::picture::embed_cover_art(&path, &png).unwrap();
        assert!(!ape_art(&path));
        assert!(ape_item(&path, "Cover Art (Back)"));
        assert_eq!(
            crate::picture::extract_cover_art(&path).unwrap().unwrap().0,
            png
        );

        let mut ape = ApeTag::new();
        ape.insert(
            ApeItem::new(
                "Cover Art (Front)".into(),
                ItemValue::Binary(b"c.jpg\0\xFF\xD8\xFF\xE0\x01\x02".to_vec()),
            )
            .unwrap(),
        );
        ape.save_to_path(&path, WriteOptions::default()).unwrap();
        crate::picture::remove_cover_art(&path).unwrap();
        assert!(!ape_art(&path));
        assert!(!ape_item(&path, "Cover Art (Back)"));
        assert!(crate::picture::extract_cover_art(&path).unwrap().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unsupported_ogg_streams_are_refused_not_retagged() {
        let (dir, _) = temp_flac(&[]);
        let path = dir.join("test.oga");
        let mut frame = vec![0xFF, 0xFB, 0x90, 0x00];
        frame.resize(417, 0);
        let bytes = [b"OggS\0\x02".to_vec(), frame.repeat(4)].concat();
        std::fs::write(&path, &bytes).unwrap();
        assert!(super::read_tags_fast(&path).is_err());
        assert!(super::write_tags(&path, &title("New")).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn writing_a_headerless_apev2_tag_keeps_the_audio() {
        let (dir, _) = temp_flac(&[]);
        let path = dir.join("test.mp3");
        let mut frame = vec![0xFF, 0xFB, 0x90, 0x00];
        frame.resize(417, 0x55);
        let audio = frame.repeat(4);
        let mut item = Vec::new();
        item.extend_from_slice(&3u32.to_le_bytes());
        item.extend_from_slice(&0u32.to_le_bytes());
        item.extend_from_slice(b"Title\0Old");
        let mut footer = b"APETAGEX".to_vec();
        footer.extend_from_slice(&2000u32.to_le_bytes());
        footer.extend_from_slice(&((item.len() + 32) as u32).to_le_bytes());
        footer.extend_from_slice(&1u32.to_le_bytes());
        footer.extend_from_slice(&(1u32 << 30).to_le_bytes());
        footer.extend_from_slice(&[0; 8]);
        std::fs::write(&path, [audio.clone(), item, footer].concat()).unwrap();

        super::write_tags(&path, &title("New")).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.windows(audio.len()).any(|w| w == audio.as_slice()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn mp3_ape_title_wins_over_truncated_id3v1() {
        use lofty::ape::{ApeItem, ApeTag};
        use lofty::config::WriteOptions;
        use lofty::id3::v1::Id3v1Tag;
        use lofty::tag::TagExt;

        let (dir, _) = temp_flac(&[]);
        let path = dir.join("test.mp3");
        let mut frame = vec![0xFF, 0xFB, 0x90, 0x00];
        frame.resize(417, 0);
        std::fs::write(&path, frame.repeat(4)).unwrap();
        let long = "A Really Long Song Title That Exceeds Thirty Chars";
        let mut ape = ApeTag::new();
        ape.insert(ApeItem::new("Title".into(), ItemValue::Text(long.into())).unwrap());
        ape.save_to_path(&path, WriteOptions::default()).unwrap();
        let mut v1 = Id3v1Tag::new();
        v1.set_title(long.to_string());
        v1.save_to_path(&path, WriteOptions::default()).unwrap();

        let tagged = lofty::probe::Probe::open(&path).unwrap().read().unwrap();
        assert!(tagged.tag(TagType::Id3v1).is_some() && tagged.tag(TagType::Ape).is_some());
        assert_eq!(
            super::read_tags_fast(&path).unwrap().title.as_deref(),
            Some(long)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn temp_mp3(
        tag: &lofty::id3::v2::Id3v2Tag,
        v23: bool,
    ) -> (std::path::PathBuf, std::path::PathBuf) {
        use lofty::tag::TagExt;
        let mut frame = vec![0xFF, 0xFB, 0x90, 0x00];
        frame.resize(417, 0);
        let (dir, path) = temp_file("t.mp3", &frame.repeat(4));
        let options = lofty::config::WriteOptions::default().use_id3v23(v23);
        tag.save_to_path(&path, options).unwrap();
        (dir, path)
    }

    fn id3v2(path: &std::path::Path) -> lofty::id3::v2::Id3v2Tag {
        super::MpegFile::read_from(
            &mut std::fs::File::open(path).unwrap(),
            lofty::config::ParseOptions::new(),
        )
        .unwrap()
        .remove_id3v2()
        .unwrap()
    }

    #[test]
    fn write_tags_keeps_txxx_keys_that_shadow_item_keys_custom() {
        let mut tag = lofty::id3::v2::Id3v2Tag::new();
        for key in ["BPM", "SCRIPT", "MOOD"] {
            tag.insert_user_text(key.into(), "old".into());
        }
        let (dir, path) = temp_mp3(&tag, false);
        let extra = super::read_tags_fast(&path).unwrap().extra;
        for key in ["BPM", "SCRIPT", "MOOD"] {
            assert_eq!(extra.get(key).map(String::as_str), Some("old"), "{key}");
        }

        let set = |value: Option<&str>| crate::types::TagWriteChanges {
            extra: Some(
                ["BPM", "SCRIPT", "mood"]
                    .map(|k| (k.to_string(), value.map(str::to_string)))
                    .into(),
            ),
            ..Default::default()
        };
        super::write_tags(&path, &set(Some("new"))).unwrap();
        let tag = id3v2(&path);
        for key in ["BPM", "SCRIPT", "MOOD"] {
            assert_eq!(tag.get_user_text(key), Some("new"), "{key}");
        }
        assert!(!super::read_tags_fast(&path)
            .unwrap()
            .extra
            .contains_key("Mood"));

        super::write_tags(&path, &set(None)).unwrap();
        assert!(super::read_tags_fast(&path).unwrap().extra.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vorbis_custom_writes_keep_multi_value_order() {
        use lofty::ogg::tag::VorbisComments;
        use lofty::tag::TagExt;

        let (dir, path) = temp_flac(&[]);
        let mut comments = VorbisComments::default();
        comments.push("CUSTOM".into(), "a".into());
        comments.push("CUSTOM".into(), "b".into());
        comments.push("MYKEY".into(), "v".into());
        comments.push("encoder".into(), "x".into());
        comments
            .save_to_path(&path, lofty::config::WriteOptions::default())
            .unwrap();

        let custom_values = || {
            let tagged = lofty::probe::Probe::open(&path).unwrap().read().unwrap();
            let native = super::Native::load(
                &path,
                tagged.file_type(),
                &Tag::new(TagType::VorbisComments),
                lofty::config::ParseOptions::new(),
            );
            let Ok(Some(super::Native::Vorbis(comments))) = native else {
                panic!("no vorbis comments");
            };
            comments
                .get_all("CUSTOM")
                .map(str::to_string)
                .collect::<Vec<_>>()
        };
        for changes in [custom(None), custom(Some("w")), title("New"), custom(None)] {
            super::write_tags(&path, &changes).unwrap();
            assert_eq!(custom_values(), ["a", "b"]);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn modify_tags_removes_lyrics_and_large_values() {
        let big = "x".repeat(super::MAX_TAG_VALUE_BYTES + 1);
        let (dir, path) = temp_flac(&[
            (ItemKey::TrackTitle, "Song"),
            (ItemKey::UnsyncLyrics, "la la"),
            (ItemKey::Description, &big),
        ]);

        super::modify_tags(&path, |t| t.title = Some("New".into())).unwrap();
        assert_eq!(
            vorbis(&path).get_string(ItemKey::Description),
            Some(big.as_str())
        );

        super::modify_tags(&path, |t| t.extra.clear()).unwrap();
        let tag = vorbis(&path);
        assert_eq!(tag.get_string(ItemKey::UnsyncLyrics), None);
        assert_eq!(tag.get_string(ItemKey::Description), None);
        assert_eq!(tag.get_string(ItemKey::TrackTitle), Some("New"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn years_beyond_four_digits_are_refused_not_truncated() {
        let (dir, path) = temp_mp3(&lofty::id3::v2::Id3v2Tag::new(), false);
        let year = |y| crate::types::TagWriteChanges {
            year: Some(Some(y)),
            ..Default::default()
        };
        assert!(super::write_tags(&path, &year(12345)).is_err());
        super::write_tags(&path, &year(2016)).unwrap();
        assert_eq!(super::read_tags_fast(&path).unwrap().year, Some(2016));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn id3v23_files_keep_frames_that_only_exist_in_id3v24() {
        let text = |id: &[u8], value: &str| {
            let body = [&[0u8][..], value.as_bytes()].concat();
            [id, &(body.len() as u32).to_be_bytes()[..], &[0, 0], &body].concat()
        };
        let frames = [text(b"TIT2", "Song"), text(b"TSOP", "Artist, The")].concat();
        let size = frames.len() as u32;
        let syncsafe = [
            (size >> 21) & 0x7F,
            (size >> 14) & 0x7F,
            (size >> 7) & 0x7F,
            size & 0x7F,
        ]
        .map(|b| b as u8);
        let mut frame = vec![0xFF, 0xFB, 0x90, 0x00];
        frame.resize(417, 0);
        let bytes = [
            &b"ID3\x03\x00\x00"[..],
            &syncsafe,
            &frames,
            &frame.repeat(4),
        ]
        .concat();
        let (dir, path) = temp_file("t.mp3", &bytes);

        let mood = crate::types::TagWriteChanges {
            extra: Some([("MOOD".to_string(), Some("Happy".to_string()))].into()),
            ..title("New")
        };
        super::write_tags(&path, &mood).unwrap();
        let tag = id3v2(&path);
        assert_eq!(tag.title().as_deref(), Some("New"));
        assert!(tag.iter().any(|f| f.id_str() == "TSOP"));
        assert_eq!(
            super::read_tags_fast(&path)
                .unwrap()
                .extra
                .get("Mood")
                .map(String::as_str),
            Some("Happy")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn id3v23_files_stay_id3v23() {
        use lofty::id3::v2::Id3v2Version;

        let mut tag = lofty::id3::v2::Id3v2Tag::new();
        tag.set_title("Old".into());
        let (dir, path) = temp_mp3(&tag, true);
        assert_eq!(id3v2(&path).original_version(), Id3v2Version::V3);

        super::write_tags(&path, &title("New")).unwrap();
        assert_eq!(id3v2(&path).original_version(), Id3v2Version::V3);
        super::write_tags(&path, &custom(Some("x"))).unwrap();
        let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0];
        crate::picture::embed_cover_art(&path, &png).unwrap();
        assert_eq!(id3v2(&path).original_version(), Id3v2Version::V3);
        crate::picture::remove_cover_art(&path).unwrap();
        let tag = id3v2(&path);
        assert_eq!(tag.original_version(), Id3v2Version::V3);
        assert_eq!(tag.title().as_deref(), Some("New"));
        assert_eq!(tag.get_user_text("MYKEY"), Some("x"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
