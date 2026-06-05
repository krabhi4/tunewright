use crate::types::{TagData, TagWriteChanges, TunewrightError, WriteResult};
use lofty::config::{ParseOptions, ParsingMode, WriteOptions};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::{Accessor, ItemKey, ItemValue, Tag, TagItem, TagType};
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

/// Read tags FAST — skips audio properties and cover art data.
/// Returns tag text fields only (title, artist, album, etc.).
/// Use this for populating the grid quickly.
pub fn read_tags_fast(path: &Path) -> Result<TagData, TunewrightError> {
    let tagged = Probe::open(path)
        .map_err(|e| TunewrightError::TagReadError(format!("{}: {}", path.display(), e)))?
        .options(fast_parse_options())
        .read()
        .map_err(|e| TunewrightError::TagReadError(format!("{}: {}", path.display(), e)))?;

    let tag_types: Vec<String> = tagged
        .tags()
        .iter()
        .map(|t| format!("{:?}", t.tag_type()))
        .collect();

    let tags: Vec<&Tag> = tagged.tags().iter().collect();

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

    let extra = collect_extra_tags(&tags);

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
    let tagged = Probe::open(path)
        .map_err(|e| TunewrightError::TagReadError(format!("{}: {}", path.display(), e)))?
        .options(full_parse_options())
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

    let tags: Vec<&Tag> = tagged.tags().iter().collect();

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

    let extra = collect_extra_tags(&tags);

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

/// Write tag changes to a single audio file
pub fn write_tags(path: &Path, changes: &TagWriteChanges) -> Result<(), TunewrightError> {
    let _lock = crate::locks::lock_file(path);
    let mut tagged = Probe::open(path)
        .map_err(|e| TunewrightError::TagWriteError(format!("{}: {}", path.display(), e)))?
        .read()
        .map_err(|e| TunewrightError::TagWriteError(format!("{}: {}", path.display(), e)))?;

    let primary_type = tagged
        .primary_tag()
        .map(|t| t.tag_type())
        .unwrap_or_else(|| tagged.primary_tag_type());

    // Collect and remove all secondary tags
    let secondary_types: Vec<TagType> = tagged
        .tags()
        .iter()
        .map(|t| t.tag_type())
        .filter(|&t| t != primary_type)
        .collect();

    let mut secondary_tags = Vec::new();
    for t_type in secondary_types {
        if let Some(t) = tagged.remove(t_type) {
            secondary_tags.push(t);
        }
        let _ = t_type.remove_from_path(path);
    }

    // Get the primary tag (inserting a new one if not present)
    let tag = match tagged.tag_mut(primary_type) {
        Some(t) => t,
        None => {
            tagged.insert_tag(Tag::new(primary_type));
            tagged.tag_mut(primary_type).unwrap()
        }
    };

    // Merge secondary tags' items into the primary tag
    for sec_tag in &secondary_tags {
        for item in sec_tag.items() {
            if !tag.items().any(|i| i.key() == item.key()) {
                tag.push(item.clone());
            }
        }
        if tag.pictures().is_empty() && !sec_tag.pictures().is_empty() {
            for pic in sec_tag.pictures() {
                tag.push_picture(pic.clone());
            }
        }
    }

    if let Some(ref v) = changes.title {
        tag.set_title(v.clone());
    }
    if let Some(ref v) = changes.artist {
        tag.set_artist(v.clone());
    }
    if let Some(ref v) = changes.album {
        tag.set_album(v.clone());
    }
    if let Some(ref v) = changes.genre {
        tag.set_genre(v.clone());
    }
    if let Some(ref v) = changes.comment {
        tag.set_comment(v.clone());
    }
    if let Some(v) = changes.year {
        // RecordingDate is the cross-format date key; ItemKey::Year isn't mapped for ID3v2.
        tag.remove_key(ItemKey::Year);
        tag.remove_key(ItemKey::RecordingDate);
        tag.push(TagItem::new(
            ItemKey::RecordingDate,
            ItemValue::Text(v.to_string()),
        ));
    }
    if let Some(v) = changes.track_number {
        tag.set_track(v);
    }
    if let Some(v) = changes.track_total {
        if primary_type != TagType::Id3v2 || changes.track_number.or_else(|| tag.track()).is_some() {
            tag.set_track_total(v);
        }
    }
    if let Some(v) = changes.disc_number {
        tag.set_disk(v);
    }
    if let Some(v) = changes.disc_total {
        if primary_type != TagType::Id3v2 || changes.disc_number.or_else(|| tag.disk()).is_some() {
            tag.set_disk_total(v);
        }
    }

    // Write album_artist via TagItem (not available on Accessor trait)
    if let Some(ref v) = changes.album_artist {
        tag.remove_key(ItemKey::AlbumArtist);
        if !v.is_empty() {
            tag.push(TagItem::new(
                ItemKey::AlbumArtist,
                ItemValue::Text(v.clone()),
            ));
        }
    }

    // Write composer via TagItem
    if let Some(ref v) = changes.composer {
        tag.remove_key(ItemKey::Composer);
        if !v.is_empty() {
            tag.push(TagItem::new(ItemKey::Composer, ItemValue::Text(v.clone())));
        }
    }

    // Write extra/custom tag fields
    if let Some(ref extra) = changes.extra {
        for (key, value) in extra {
            let Some(item_key) = string_to_item_key(key) else {
                continue;
            };
            tag.remove_key(item_key);
            if !value.is_empty() {
                tag.push(TagItem::new(item_key, ItemValue::Text(value.clone())));
            }
        }
    }

    tagged
        .save_to_path(path, WriteOptions::default())
        .map_err(|e| TunewrightError::TagWriteError(format!("{}: {}", path.display(), e)))?;

    Ok(())
}

pub fn batch_write_tags(changes: &[(String, PathBuf, TagWriteChanges)]) -> Vec<WriteResult> {
    changes
        .iter()
        .map(
            |(id, canonical_path, ch)| match write_tags(canonical_path, ch) {
                Ok(()) => WriteResult {
                    id: id.clone(),
                    status: "ok".to_string(),
                    error: None,
                },
                Err(e) => WriteResult {
                    id: id.clone(),
                    status: "error".to_string(),
                    error: Some(e.to_string()),
                },
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

/// Convert a string key back to an ItemKey for writing
fn string_to_item_key(key: &str) -> Option<ItemKey> {
    Some(match key {
        // Titles
        "AlbumTitle" => ItemKey::AlbumTitle,
        "SetSubtitle" => ItemKey::SetSubtitle,
        "ShowName" => ItemKey::ShowName,
        "ContentGroup" => ItemKey::ContentGroup,
        "TrackTitle" => ItemKey::TrackTitle,
        "TrackSubtitle" => ItemKey::TrackSubtitle,

        // Original names
        "OriginalAlbumTitle" => ItemKey::OriginalAlbumTitle,
        "OriginalArtist" => ItemKey::OriginalArtist,
        "OriginalLyricist" => ItemKey::OriginalLyricist,

        // Sorting
        "AlbumTitleSortOrder" => ItemKey::AlbumTitleSortOrder,
        "AlbumArtistSortOrder" => ItemKey::AlbumArtistSortOrder,
        "TrackTitleSortOrder" => ItemKey::TrackTitleSortOrder,
        "TrackArtistSortOrder" => ItemKey::TrackArtistSortOrder,
        "ShowNameSortOrder" => ItemKey::ShowNameSortOrder,
        "ComposerSortOrder" => ItemKey::ComposerSortOrder,

        // People & Organizations
        "AlbumArtist" => ItemKey::AlbumArtist,
        "AlbumArtists" => ItemKey::AlbumArtists,
        "TrackArtist" => ItemKey::TrackArtist,
        "TrackArtists" => ItemKey::TrackArtists,
        "Arranger" => ItemKey::Arranger,
        "Writer" => ItemKey::Writer,
        "Composer" => ItemKey::Composer,
        "Conductor" => ItemKey::Conductor,
        "Director" => ItemKey::Director,
        "Engineer" => ItemKey::Engineer,
        "Lyricist" => ItemKey::Lyricist,
        "MixDj" => ItemKey::MixDj,
        "MixEngineer" => ItemKey::MixEngineer,
        "Performer" => ItemKey::Performer,
        "Producer" => ItemKey::Producer,
        "Publisher" => ItemKey::Publisher,
        "Label" => ItemKey::Label,
        "InternetRadioStationName" => ItemKey::InternetRadioStationName,
        "InternetRadioStationOwner" => ItemKey::InternetRadioStationOwner,
        "Remixer" => ItemKey::Remixer,

        // Counts & Indexes
        "DiscNumber" => ItemKey::DiscNumber,
        "DiscTotal" => ItemKey::DiscTotal,
        "TrackNumber" => ItemKey::TrackNumber,
        "TrackTotal" => ItemKey::TrackTotal,
        "Popularimeter" => ItemKey::Popularimeter,
        "ParentalAdvisory" => ItemKey::ParentalAdvisory,

        // Dates
        "RecordingDate" => ItemKey::RecordingDate,
        "Year" => ItemKey::Year,
        "ReleaseDate" => ItemKey::ReleaseDate,
        "OriginalReleaseDate" => ItemKey::OriginalReleaseDate,

        // Identifiers
        "Isrc" => ItemKey::Isrc,
        "Barcode" => ItemKey::Barcode,
        "AcoustId" => ItemKey::AcoustId,
        "AcoustIdFingerprint" => ItemKey::AcoustIdFingerprint,
        "CatalogNumber" => ItemKey::CatalogNumber,
        "Work" => ItemKey::Work,
        "Movement" => ItemKey::Movement,
        "MovementNumber" => ItemKey::MovementNumber,
        "MovementTotal" => ItemKey::MovementTotal,
        "ReleaseCountry" => ItemKey::ReleaseCountry,

        // MusicBrainz Identifiers
        "MusicBrainzRecordingId" => ItemKey::MusicBrainzRecordingId,
        "MusicBrainzTrackId" => ItemKey::MusicBrainzTrackId,
        "MusicBrainzReleaseId" => ItemKey::MusicBrainzReleaseId,
        "MusicBrainzReleaseGroupId" => ItemKey::MusicBrainzReleaseGroupId,
        "MusicBrainzArtistId" => ItemKey::MusicBrainzArtistId,
        "MusicBrainzReleaseArtistId" => ItemKey::MusicBrainzReleaseArtistId,
        "MusicBrainzWorkId" => ItemKey::MusicBrainzWorkId,
        "MusicBrainzReleaseType" => ItemKey::MusicBrainzReleaseType,

        // Flags
        "FlagCompilation" => ItemKey::FlagCompilation,
        "FlagPodcast" => ItemKey::FlagPodcast,

        // File Information
        "FileOwner" => ItemKey::FileOwner,
        "TaggingTime" => ItemKey::TaggingTime,
        "Length" => ItemKey::Length,
        "OriginalFileName" => ItemKey::OriginalFileName,
        "OriginalMediaType" => ItemKey::OriginalMediaType,

        // Encoder information
        "EncodedBy" => ItemKey::EncodedBy,
        "EncoderSoftware" => ItemKey::EncoderSoftware,
        "EncoderSettings" => ItemKey::EncoderSettings,
        "EncodingTime" => ItemKey::EncodingTime,
        "ReplayGainAlbumGain" => ItemKey::ReplayGainAlbumGain,
        "ReplayGainAlbumPeak" => ItemKey::ReplayGainAlbumPeak,
        "ReplayGainTrackGain" => ItemKey::ReplayGainTrackGain,
        "ReplayGainTrackPeak" => ItemKey::ReplayGainTrackPeak,

        // URLs
        "AudioFileUrl" => ItemKey::AudioFileUrl,
        "AudioSourceUrl" => ItemKey::AudioSourceUrl,
        "CommercialInformationUrl" => ItemKey::CommercialInformationUrl,
        "CopyrightUrl" => ItemKey::CopyrightUrl,
        "TrackArtistUrl" => ItemKey::TrackArtistUrl,
        "RadioStationUrl" => ItemKey::RadioStationUrl,
        "PaymentUrl" => ItemKey::PaymentUrl,
        "PublisherUrl" => ItemKey::PublisherUrl,

        // Style
        "Genre" => ItemKey::Genre,
        "InitialKey" => ItemKey::InitialKey,
        "Color" => ItemKey::Color,
        "Mood" => ItemKey::Mood,
        "Bpm" => ItemKey::Bpm,
        "IntegerBpm" => ItemKey::IntegerBpm,

        // Legal
        "CopyrightMessage" => ItemKey::CopyrightMessage,
        "License" => ItemKey::License,

        // Podcast
        "PodcastDescription" => ItemKey::PodcastDescription,
        "PodcastSeriesCategory" => ItemKey::PodcastSeriesCategory,
        "PodcastUrl" => ItemKey::PodcastUrl,
        "PodcastGlobalUniqueId" => ItemKey::PodcastGlobalUniqueId,
        "PodcastKeywords" => ItemKey::PodcastKeywords,

        // Miscellaneous
        "Comment" => ItemKey::Comment,
        "Description" => ItemKey::Description,
        "Language" => ItemKey::Language,
        "Script" => ItemKey::Script,
        "Lyrics" => ItemKey::Lyrics,
        "UnsyncLyrics" => ItemKey::UnsyncLyrics,

        // Vendor-specific
        "AppleXid" => ItemKey::AppleXid,
        "AppleId3v2ContentGroup" => ItemKey::AppleId3v2ContentGroup,
        _ => return None,
    })
}

/// Collect all non-standard tag items into a HashMap
fn collect_extra_tags(tags: &[&Tag]) -> HashMap<String, String> {
    let mut extra = HashMap::new();
    for tag in tags {
        for item in tag.items() {
            if is_standard_key(item.key()) {
                continue;
            }
            let key = item_key_to_string(item.key());
            if extra.contains_key(&key) {
                continue; // first tag wins
            }
            if let ItemValue::Text(val) = item.value() {
                if !val.is_empty() {
                    extra.insert(key, val.to_string());
                }
            }
        }
    }
    extra
}

#[cfg(test)]
mod tests {
    use super::{parse_year, read_year};
    use lofty::tag::{Accessor, ItemKey, ItemValue, Tag, TagItem, TagType};
    use lofty::file::{AudioFile, TaggedFileExt};

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
    fn test_write_tags_removes_and_merges_secondary_tags() {
        use std::fs::File;
        use std::io::Write;
        use lofty::probe::Probe;
        use lofty::tag::{Tag, TagItem, ItemKey, ItemValue, TagType};
        use lofty::config::WriteOptions;
        use crate::types::TagWriteChanges;

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
        primary_tag.push(TagItem::new(ItemKey::TrackTitle, ItemValue::Text("Primary Title".to_string())));
        primary_tag.push(TagItem::new(ItemKey::TrackArtist, ItemValue::Text("Primary Artist".to_string())));
        tagged.insert_tag(primary_tag);

        let mut secondary_tag = Tag::new(secondary_type);
        secondary_tag.push(TagItem::new(ItemKey::TrackTitle, ItemValue::Text("Secondary Title".to_string())));
        secondary_tag.push(TagItem::new(ItemKey::Composer, ItemValue::Text("Secondary Composer".to_string())));
        tagged.insert_tag(secondary_tag);

        tagged.save_to_path(&file_path, WriteOptions::default()).unwrap();

        // 2. Call write_tags to write changes and merge secondary tags
        let mut changes = TagWriteChanges::default();
        changes.artist = Some("New Artist".to_string());
        super::write_tags(&file_path, &changes).unwrap();

        // 3. Verify the result
        let tagged_after = Probe::open(&file_path).unwrap().read().unwrap();
        
        // Assert that the secondary tag was completely removed
        assert!(tagged_after.tag(secondary_type).is_none());

        // Assert that the primary tag exists and has the correct merged/updated fields
        let primary_after = tagged_after.tag(primary_type).unwrap();
        
        assert_eq!(
            primary_after.get_string(ItemKey::TrackTitle),
            Some("Primary Title")
        );

        assert_eq!(
            primary_after.get_string(ItemKey::TrackArtist),
            Some("New Artist")
        );

        assert_eq!(
            primary_after.get_string(ItemKey::Composer),
            Some("Secondary Composer")
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_write_tags_id3v2_total_only_no_fabrication() {
        use std::fs::File;
        use std::io::Write;
        use lofty::probe::Probe;
        use lofty::tag::{Tag, TagType};
        use lofty::config::WriteOptions;
        use lofty::file::AudioFile;
        use crate::types::TagWriteChanges;

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
            tagged.save_to_path(&file_path, WriteOptions::default()).unwrap();
        }

        // 1. Write only track_total and disc_total (no track_number or disc_number)
        let mut changes = TagWriteChanges::default();
        changes.track_total = Some(12);
        changes.disc_total = Some(2);
        super::write_tags(&file_path, &changes).unwrap();

        // 2. Read back and verify — no "0/N" fabrication
        let tagged_after = Probe::open(&file_path).unwrap().read().unwrap();
        if let Some(tag) = tagged_after.tag(TagType::Id3v2) {
            // Assert track number is NOT fabricated to 0
            assert_eq!(tag.track(), None, "track number must not be fabricated");
            assert_eq!(tag.disk(), None, "disc number must not be fabricated");
            // Since no track_number was present, track_total should also be absent in ID3v2
            assert_eq!(tag.track_total(), None, "track_total must not be written without track_number in ID3v2");
            assert_eq!(tag.disk_total(), None, "disc_total must not be written without disc_number in ID3v2");
        }

        // 3. Now write both track number AND track total together
        let mut changes2 = TagWriteChanges::default();
        changes2.track_number = Some(3);
        changes2.track_total = Some(12);
        changes2.disc_number = Some(1);
        changes2.disc_total = Some(2);
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
                Some(key.clone()),
                "Failed to parse debug string of {:?} back to ItemKey",
                key
            );
        }
    }
}

