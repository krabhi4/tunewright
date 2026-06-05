use crate::types::{AudioFormat, DirNode, FileEntry, FileListResult, TunewrightError};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

/// Generate a deterministic ID from a relative path
pub fn file_id(relative_path: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(relative_path.as_bytes());
    let result = hasher.finalize();
    hex::encode(&result[..12]) // 24-char hex string
}

/// Resolve a user-provided path safely within the data root.
/// Prevents path traversal attacks.
pub fn resolve_safe_path(data_root: &Path, requested: &str) -> Result<PathBuf, TunewrightError> {
    let clean = requested.trim_start_matches('/');
    let candidate = data_root.join(clean);

    let resolved = candidate
        .canonicalize()
        .map_err(|_| TunewrightError::FileNotFound(candidate.clone()))?;

    let root_canonical = data_root.canonicalize().map_err(TunewrightError::Io)?;

    if !resolved.starts_with(&root_canonical) {
        return Err(TunewrightError::PathTraversal(requested.to_string()));
    }

    Ok(resolved)
}

/// Scan a directory for supported audio files. Does NOT read tags (fast).
pub fn scan_directory(
    data_root: &Path,
    relative_path: &str,
    offset: usize,
    limit: usize,
) -> Result<FileListResult, TunewrightError> {
    let dir = resolve_safe_path(data_root, relative_path)?;

    if !dir.is_dir() {
        return Err(TunewrightError::FileNotFound(dir));
    }

    let root_canonical = data_root.canonicalize()?;

    let mut files: Vec<FileEntry> = Vec::new();
    let mut directories: Vec<String> = Vec::new();

    let mut entries: Vec<_> = fs::read_dir(&dir)?.filter_map(|e| e.ok()).collect();

    entries.sort_by_key(|a| a.file_name());

    for entry in &entries {
        let path = entry.path();
        let metadata = entry.metadata().ok();

        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                directories.push(name.to_string());
            }
            continue;
        }

        // Skip hidden files (including transient .tw-tmp-* atomic-write copies)
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with('.'))
        {
            continue;
        }

        let ext = match path.extension().and_then(|e| e.to_str()) {
            Some(e) => e,
            None => continue,
        };

        let format = match AudioFormat::from_extension(ext) {
            Some(f) => f,
            None => continue,
        };

        // `dir` is already canonical (from resolve_safe_path), so for a regular
        // file `path` is canonical too and we can strip the root prefix directly,
        // avoiding a canonicalize() syscall per file. Symlinks still get resolved
        // so their target is re-checked against the root.
        let is_symlink = metadata
            .as_ref()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false);
        let relative = if is_symlink {
            match path.canonicalize().ok().and_then(|p| {
                p.strip_prefix(&root_canonical)
                    .ok()
                    .map(|r| r.to_path_buf())
            }) {
                Some(r) => r,
                None => continue, // skip symlinks that resolve outside root / are broken
            }
        } else {
            match path.strip_prefix(&root_canonical) {
                Ok(r) => r.to_path_buf(),
                Err(_) => continue,
            }
        };

        let relative_str = relative.to_string_lossy().to_string();
        let id = file_id(&relative_str);

        let filename = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);

        let modified_at = metadata
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .map(|d| chrono_format_timestamp(d.as_secs()))
            })
            .unwrap_or_default();

        // Skip probing files here for speed — duration and cover art
        // will be fetched lazily via the tags endpoint
        files.push(FileEntry {
            id,
            filename,
            relative_path: relative_str,
            format,
            format_label: format.display_name().to_string(),
            size,
            duration_secs: None,
            has_cover: false,
            modified_at,
        });
    }

    let total_dirs = directories.len();
    let total_files = files.len();
    let total = total_dirs + total_files;

    let paginated_dirs: Vec<String> = if offset < total_dirs {
        directories.into_iter().skip(offset).take(limit).collect()
    } else {
        Vec::new()
    };

    let files_skip = offset.saturating_sub(total_dirs);

    let files_limit = if paginated_dirs.is_empty() {
        limit
    } else {
        limit - paginated_dirs.len()
    };

    let paginated_files: Vec<FileEntry> = files
        .into_iter()
        .skip(files_skip)
        .take(files_limit)
        .collect();

    Ok(FileListResult {
        path: relative_path.to_string(),
        files: paginated_files,
        total,
        directories: paginated_dirs,
    })
}

/// Build a directory tree starting from data_root
pub fn build_dir_tree(data_root: &Path, max_depth: usize) -> Result<DirNode, TunewrightError> {
    let root_canonical = data_root.canonicalize()?;
    let root_name = root_canonical
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    fn walk(dir: &Path, root: &Path, depth: usize, max_depth: usize) -> Vec<DirNode> {
        if depth >= max_depth {
            return Vec::new();
        }

        let mut children: Vec<DirNode> = Vec::new();

        let mut entries: Vec<_> = fs::read_dir(dir)
            .ok()
            .map(|rd| rd.filter_map(|e| e.ok()).collect())
            .unwrap_or_default();

        entries.sort_by_key(|a| a.file_name());

        for entry in entries {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            // Skip hidden directories
            if name.starts_with('.') {
                continue;
            }

            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();

            let sub_children = walk(&path, root, depth + 1, max_depth);

            children.push(DirNode {
                name,
                path: relative,
                children: sub_children,
            });
        }

        children
    }

    let children = walk(&root_canonical, &root_canonical, 0, max_depth);

    Ok(DirNode {
        name: root_name,
        path: String::new(),
        children,
    })
}

/// Format a unix timestamp as ISO 8601 (without pulling in chrono)
fn chrono_format_timestamp(secs: u64) -> String {
    // Simple UTC ISO 8601 formatting
    let s = secs.min(253402300799);
    let days = s / 86400;
    let time_of_day = s % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Days since epoch to Y-M-D (simplified)
    let mut y = 1970i64;
    let mut remaining_days = days as i64;

    loop {
        let days_in_year = if is_leap_year(y) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        y += 1;
    }

    let days_in_months: [i64; 12] = if is_leap_year(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut m = 0;
    for (i, &dim) in days_in_months.iter().enumerate() {
        if remaining_days < dim {
            m = i + 1;
            break;
        }
        remaining_days -= dim;
    }
    if m == 0 {
        m = 12;
    }
    let d = remaining_days + 1;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y, m, d, hours, minutes, seconds
    )
}

fn is_leap_year(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_id_deterministic() {
        let id1 = file_id("music/album/track.mp3");
        let id2 = file_id("music/album/track.mp3");
        assert_eq!(id1, id2);
        assert_eq!(id1.len(), 24);
    }

    #[test]
    fn test_file_id_different_paths() {
        let id1 = file_id("a.mp3");
        let id2 = file_id("b.mp3");
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_timestamp_formatting() {
        assert_eq!(chrono_format_timestamp(0), "1970-01-01T00:00:00Z");
        assert_eq!(chrono_format_timestamp(1704067200), "2024-01-01T00:00:00Z");

        // Far-future date (9999-12-31T23:59:59Z)
        assert_eq!(
            chrono_format_timestamp(253402300799),
            "9999-12-31T23:59:59Z"
        );
        // u64::MAX should be capped and not hang
        assert_eq!(chrono_format_timestamp(u64::MAX), "9999-12-31T23:59:59Z");
    }

    #[test]
    fn test_audio_format_from_extension() {
        assert_eq!(AudioFormat::from_extension("mp3"), Some(AudioFormat::Mp3));
        assert_eq!(AudioFormat::from_extension("FLAC"), Some(AudioFormat::Flac));
        assert_eq!(AudioFormat::from_extension("m4a"), Some(AudioFormat::Mp4));
        assert_eq!(AudioFormat::from_extension("txt"), None);
    }

    #[test]
    fn test_scan_directory_pagination() {
        fn rand_num() -> u64 {
            use std::time::{SystemTime, UNIX_EPOCH};
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64
        }

        let temp_dir = std::env::temp_dir().join(format!("tunewright_scan_{}", rand_num()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Create 3 subdirectories
        std::fs::create_dir(temp_dir.join("dir_a")).unwrap();
        std::fs::create_dir(temp_dir.join("dir_b")).unwrap();
        std::fs::create_dir(temp_dir.join("dir_c")).unwrap();

        // Create 3 audio files
        use std::io::Write;
        let wav_bytes = b"RIFF\x28\x00\x00\x00WAVEfmt \x10\x00\x00\x00\x01\x00\x01\x00\x44\xac\x00\x00\x88\x58\x01\x00\x02\x00\x10\x00data\x04\x00\x00\x00\x00\x00\x00\x00";
        std::fs::File::create(temp_dir.join("track_1.mp3"))
            .unwrap()
            .write_all(wav_bytes)
            .unwrap();
        std::fs::File::create(temp_dir.join("track_2.wav"))
            .unwrap()
            .write_all(wav_bytes)
            .unwrap();
        std::fs::File::create(temp_dir.join("track_3.flac"))
            .unwrap()
            .write_all(wav_bytes)
            .unwrap();

        // Call scan_directory
        // Total items: 3 directories + 3 files = 6 items.
        // Directories: dir_a, dir_b, dir_c (alphabetical)
        // Files: track_1.mp3, track_2.wav, track_3.flac (alphabetical)

        // Test 1: Page 1, limit 2
        let res1 = scan_directory(&temp_dir, "", 0, 2).unwrap();
        assert_eq!(res1.total, 6);
        assert_eq!(
            res1.directories,
            vec!["dir_a".to_string(), "dir_b".to_string()]
        );
        assert!(res1.files.is_empty());

        // Test 2: Page 2, offset 2, limit 2
        let res2 = scan_directory(&temp_dir, "", 2, 2).unwrap();
        assert_eq!(res2.total, 6);
        assert_eq!(res2.directories, vec!["dir_c".to_string()]);
        assert_eq!(res2.files.len(), 1);
        assert_eq!(res2.files[0].filename, "track_1.mp3");

        // Test 3: Page 3, offset 4, limit 2
        let res3 = scan_directory(&temp_dir, "", 4, 2).unwrap();
        assert_eq!(res3.total, 6);
        assert!(res3.directories.is_empty());
        assert_eq!(res3.files.len(), 2);
        assert_eq!(res3.files[0].filename, "track_2.wav");
        assert_eq!(res3.files[1].filename, "track_3.flac");

        // Test 4: Page 4, offset 6, limit 2
        let res4 = scan_directory(&temp_dir, "", 6, 2).unwrap();
        assert_eq!(res4.total, 6);
        assert!(res4.directories.is_empty());
        assert!(res4.files.is_empty());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
