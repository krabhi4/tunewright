use crate::audio;
use crate::format_string;
use crate::types::TunewrightError;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenamePreview {
    pub id: String,
    pub old_name: String,
    pub new_name: String,
    pub conflict: bool,
}

/// Preview renames without executing
pub fn preview_renames(
    _data_root: &Path,
    files: &[(String, String, PathBuf)], // (id, relative_path, canonical_path)
    format: &str,
) -> Result<Vec<RenamePreview>, TunewrightError> {
    // Reads are independent, so compute (id, old_name, new_name) in parallel.
    // Conflict detection is order-dependent, so it runs as a sequential pass below.
    let computed: Vec<(String, String, String)> = files
        .par_iter()
        .map(|(id, _rel_path, canonical_path)| {
            let old_name = canonical_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let extension = canonical_path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let tags = audio::read_tags_fast(canonical_path).unwrap_or_default();
            let new_stem = format_string::evaluate(format, &tags);

            let new_name = if new_stem.is_empty() {
                old_name.clone()
            } else {
                format!("{}.{}", new_stem, extension)
            };

            (id.clone(), old_name, new_name)
        })
        .collect();

    let mut previews = Vec::with_capacity(computed.len());
    let mut used_names: HashSet<String> = HashSet::new();

    for (id, old_name, new_name) in computed {
        let conflict = used_names.contains(&new_name.to_lowercase());
        used_names.insert(new_name.to_lowercase());

        previews.push(RenamePreview {
            id,
            old_name,
            new_name,
            conflict,
        });
    }

    Ok(previews)
}

/// Execute renames
pub fn execute_renames(
    data_root: &Path,
    files: &[(String, String, PathBuf)], // (id, relative_path, canonical_path)
    format: &str,
) -> Vec<RenameResult> {
    let previews = match preview_renames(data_root, files, format) {
        Ok(p) => p,
        Err(e) => {
            return files
                .iter()
                .map(|(id, rel_path, _)| RenameResult {
                    id: id.clone(),
                    status: "error".to_string(),
                    old_name: String::new(),
                    new_name: String::new(),
                    new_relative_path: rel_path.clone(),
                    error: Some(e.to_string()),
                })
                .collect();
        }
    };

    previews
        .into_iter()
        .zip(files.iter())
        .map(|(preview, (_, rel_path, canonical_path))| {
            // Where the file ends up on success vs. where it stays otherwise.
            let target_rel = rel_path_with_name(rel_path, &preview.new_name);
            let unchanged_rel = rel_path.clone();

            if preview.conflict {
                return RenameResult {
                    id: preview.id,
                    status: "error".to_string(),
                    old_name: preview.old_name,
                    new_name: preview.new_name,
                    new_relative_path: unchanged_rel,
                    error: Some("Duplicate filename conflict".to_string()),
                };
            }

            if preview.old_name == preview.new_name {
                return RenameResult {
                    id: preview.id,
                    status: "skipped".to_string(),
                    old_name: preview.old_name,
                    new_name: preview.new_name,
                    new_relative_path: unchanged_rel,
                    error: None,
                };
            }

            if preview.new_name.contains('/')
                || preview.new_name.contains('\\')
                || preview.new_name == ".."
                || preview.new_name == "."
            {
                return RenameResult {
                    id: preview.id,
                    status: "error".to_string(),
                    old_name: preview.old_name,
                    new_name: preview.new_name,
                    new_relative_path: unchanged_rel,
                    error: Some("Invalid target filename".to_string()),
                };
            }

            let old_path = canonical_path.clone();
            let new_path = old_path.with_file_name(&preview.new_name);

            // Acquire locks for both the old path and the target path to prevent TOCTOU races
            let _lock = crate::locks::lock_two_files(&old_path, &new_path);

            // Atomic collision check: hard_link fails if target already exists,
            // avoiding the TOCTOU race of exists()+rename().
            match std::fs::hard_link(&old_path, &new_path) {
                Ok(()) => {
                    // Link created — remove old name to complete the "rename"
                    if let Err(e) = std::fs::remove_file(&old_path) {
                        let _ = std::fs::remove_file(&new_path);
                        return RenameResult {
                            id: preview.id,
                            status: "error".to_string(),
                            old_name: preview.old_name,
                            new_name: preview.new_name,
                            new_relative_path: unchanged_rel,
                            error: Some(format!("Failed to remove old file: {}", e)),
                        };
                    }
                    return RenameResult {
                        id: preview.id,
                        status: "ok".to_string(),
                        old_name: preview.old_name,
                        new_name: preview.new_name,
                        new_relative_path: target_rel,
                        error: None,
                    };
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    if !is_same_file(&old_path, &new_path) {
                        return RenameResult {
                            id: preview.id,
                            status: "error".to_string(),
                            old_name: preview.old_name,
                            new_name: preview.new_name,
                            new_relative_path: unchanged_rel,
                            error: Some("Target file already exists".to_string()),
                        };
                    }
                }
                Err(_) => {
                    // hard_link fails across filesystems — fall back to rename
                }
            }

            // Fallback: standard rename (cross-filesystem or unsupported hard_link)
            if new_path.try_exists().unwrap_or(false) && !is_same_file(&old_path, &new_path) {
                return RenameResult {
                    id: preview.id,
                    status: "error".to_string(),
                    old_name: preview.old_name,
                    new_name: preview.new_name,
                    new_relative_path: unchanged_rel,
                    error: Some("Target file already exists".to_string()),
                };
            }

            match std::fs::rename(&old_path, &new_path) {
                Ok(()) => RenameResult {
                    id: preview.id,
                    status: "ok".to_string(),
                    old_name: preview.old_name,
                    new_name: preview.new_name,
                    new_relative_path: target_rel,
                    error: None,
                },
                Err(e) => RenameResult {
                    id: preview.id,
                    status: "error".to_string(),
                    old_name: preview.old_name,
                    new_name: preview.new_name,
                    new_relative_path: unchanged_rel,
                    error: Some(e.to_string()),
                },
            }
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameResult {
    pub id: String,
    pub status: String,
    pub old_name: String,
    pub new_name: String,
    /// Relative path of the file after this operation (the new location on
    /// success, otherwise the unchanged original path). Lets clients avoid
    /// re-deriving the directory portion themselves.
    pub new_relative_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Join `new_name` onto the directory portion of `rel_path`.
fn rel_path_with_name(rel_path: &str, new_name: &str) -> String {
    match Path::new(rel_path).parent() {
        Some(parent) if !parent.as_os_str().is_empty() => {
            format!("{}/{}", parent.to_string_lossy(), new_name)
        }
        _ => new_name.to_string(),
    }
}

/// Check if two paths point to the same physical file.
fn is_same_file(path1: &Path, path2: &Path) -> bool {
    match (std::fs::canonicalize(path1), std::fs::canonicalize(path2)) {
        (Ok(p1), Ok(p2)) => p1 == p2,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    #[test]
    fn test_is_same_file() {
        let temp_dir = std::env::temp_dir().join(format!("tunewright_test_{}", rand_num()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let path1 = temp_dir.join("file_a.txt");
        let path2 = temp_dir.join("file_b.txt");

        // Neither exists
        assert!(!is_same_file(&path1, &path2));

        // Create file A
        File::create(&path1).unwrap();
        assert!(is_same_file(&path1, &path1));
        assert!(!is_same_file(&path1, &path2));

        // Create file B
        File::create(&path2).unwrap();
        assert!(!is_same_file(&path1, &path2));

        // Casing tests for case-insensitive OS
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            let path_cased = temp_dir.join("Song_Case.txt");
            let path_lower = temp_dir.join("song_case.txt");
            File::create(&path_cased).unwrap();
            assert!(is_same_file(&path_cased, &path_lower));
        }

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_execute_renames_collision() {
        let temp_dir = std::env::temp_dir().join(format!("tunewright_test_{}", rand_num()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let old_rel = "old.mp3";
        let target_rel = "target.mp3";

        let old_path = temp_dir.join(old_rel);
        let target_path = temp_dir.join(target_rel);

        File::create(&old_path).unwrap();
        File::create(&target_path).unwrap();

        // Try renaming old.mp3 -> target.mp3
        let files = vec![("1".to_string(), old_rel.to_string(), old_path.clone())];
        let results = execute_renames(&temp_dir, &files, "target");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, "error");
        assert!(results[0]
            .error
            .as_ref()
            .unwrap()
            .contains("Target file already exists"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn test_execute_renames_case_only() {
        let temp_dir = std::env::temp_dir().join(format!("tunewright_test_{}", rand_num()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let old_rel = "Song.flac";
        let old_path = temp_dir.join(old_rel);

        use std::io::Write;
        let flac_bytes = b"fLaC\x80\x00\x00\x22\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";
        let mut f = File::create(&old_path).unwrap();
        f.write_all(flac_bytes).unwrap();

        // Write title so that it renames to "song" (different casing)
        let mut changes = crate::types::TagWriteChanges::default();
        changes.title = Some("song".to_string());
        crate::audio::write_tags(&old_path, &changes).unwrap();

        // Executing case-only rename Song.flac -> song.flac should succeed on case-insensitive OS
        let files = vec![("1".to_string(), old_rel.to_string(), old_path.clone())];
        let results = execute_renames(&temp_dir, &files, "%title%");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, "ok");
        assert_eq!(results[0].new_relative_path, "song.flac");

        // Verify file is renamed on disk
        assert!(temp_dir.join("song.flac").exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_execute_renames_mixed() {
        let temp_dir = std::env::temp_dir().join(format!("tunewright_test_{}", rand_num()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let file1 = "song1.flac";
        let file2 = "song2.flac";
        let file3 = "song3.flac";

        use std::io::Write;
        let flac_bytes = b"fLaC\x80\x00\x00\x22\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

        let mut f1 = File::create(temp_dir.join(file1)).unwrap();
        f1.write_all(flac_bytes).unwrap();

        let mut f2 = File::create(temp_dir.join(file2)).unwrap();
        f2.write_all(flac_bytes).unwrap();

        let mut f3 = File::create(temp_dir.join(file3)).unwrap();
        f3.write_all(flac_bytes).unwrap();

        // Staging a target file that will cause a collision for song2
        File::create(temp_dir.join("collision.flac")).unwrap();

        // Write tags so we can use a format string that resolves differently for each file
        let mut changes1 = crate::types::TagWriteChanges::default();
        changes1.title = Some("new_song1".to_string());
        crate::audio::write_tags(&temp_dir.join(file1), &changes1).unwrap();

        let mut changes2 = crate::types::TagWriteChanges::default();
        changes2.title = Some("collision".to_string());
        crate::audio::write_tags(&temp_dir.join(file2), &changes2).unwrap();

        let mut changes3 = crate::types::TagWriteChanges::default();
        changes3.title = Some("song3".to_string());
        crate::audio::write_tags(&temp_dir.join(file3), &changes3).unwrap();

        let files = vec![
            ("1".to_string(), file1.to_string(), temp_dir.join(file1)), // will rename to "new_song1.flac" (success)
            ("2".to_string(), file2.to_string(), temp_dir.join(file2)), // will try to rename to "collision.flac" (collision error)
            ("3".to_string(), file3.to_string(), temp_dir.join(file3)), // will rename to "song3.flac" (skipped because unchanged)
        ];

        let results = execute_renames(&temp_dir, &files, "%title%");

        assert_eq!(results.len(), 3);

        // Result 1: ok
        let r1 = results.iter().find(|r| r.id == "1").unwrap();
        assert_eq!(r1.status, "ok");
        assert_eq!(r1.new_relative_path, "new_song1.flac");

        // Result 2: error (Target file already exists)
        let r2 = results.iter().find(|r| r.id == "2").unwrap();
        assert_eq!(r2.status, "error");
        assert!(r2
            .error
            .as_ref()
            .unwrap()
            .contains("Target file already exists"));

        // Result 3: skipped (unchanged)
        let r3 = results.iter().find(|r| r.id == "3").unwrap();
        assert_eq!(r3.status, "skipped");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_execute_renames_path_safety_traversal_prevention() {
        let temp_dir = std::env::temp_dir().join(format!("tunewright_test_{}", rand_num()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let old_rel = "song";
        let old_path = temp_dir.join(old_rel);

        use std::io::Write;
        let flac_bytes = b"fLaC\x80\x00\x00\x22\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";
        let mut f = File::create(&old_path).unwrap();
        f.write_all(flac_bytes).unwrap();

        let files = vec![("1".to_string(), old_rel.to_string(), old_path.clone())];
        // Rename using the format string "." (which evaluates to "." and has no extension, resulting in target filename "..")
        let results = execute_renames(&temp_dir, &files, ".");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, "error");
        assert!(results[0]
            .error
            .as_ref()
            .unwrap()
            .contains("Invalid target filename"));

        // Verify old file still exists
        assert!(old_path.exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    fn rand_num() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    }

    #[test]
    fn test_execute_renames_concurrent_collision_toctou() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        let temp_dir = std::env::temp_dir().join(format!("tunewright_test_{}", rand_num()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Create two source files
        let file1 = "song1.flac";
        let file2 = "song2.flac";
        let flac_bytes = b"fLaC\x80\x00\x00\x22\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

        use std::io::Write;
        let mut f1 = File::create(temp_dir.join(file1)).unwrap();
        f1.write_all(flac_bytes).unwrap();

        let mut f2 = File::create(temp_dir.join(file2)).unwrap();
        f2.write_all(flac_bytes).unwrap();

        // Write tags so that both rename formats map to the same name: "target"
        let mut changes1 = crate::types::TagWriteChanges::default();
        changes1.title = Some("target".to_string());
        crate::audio::write_tags(&temp_dir.join(file1), &changes1).unwrap();

        let mut changes2 = crate::types::TagWriteChanges::default();
        changes2.title = Some("target".to_string());
        crate::audio::write_tags(&temp_dir.join(file2), &changes2).unwrap();

        // We run two renames concurrently using threads.
        // We synchronize them using a Barrier to maximize the chance of concurrent execution.
        let barrier = Arc::new(Barrier::new(2));
        let temp_dir_arc = Arc::new(temp_dir.clone());

        let t1_dir = temp_dir_arc.clone();
        let t1_barrier = barrier.clone();
        let files1 = vec![("1".to_string(), file1.to_string(), temp_dir.join(file1))];
        let handle1 = thread::spawn(move || {
            t1_barrier.wait();
            execute_renames(&t1_dir, &files1, "%title%")
        });

        let t2_dir = temp_dir_arc.clone();
        let t2_barrier = barrier.clone();
        let files2 = vec![("2".to_string(), file2.to_string(), temp_dir.join(file2))];
        let handle2 = thread::spawn(move || {
            t2_barrier.wait();
            execute_renames(&t2_dir, &files2, "%title%")
        });

        let r1 = handle1.join().unwrap();
        let r2 = handle2.join().unwrap();

        // We expect one of them to succeed, and the other to return "Target file already exists"
        let s1 = &r1[0];
        let s2 = &r2[0];

        if s1.status == "ok" {
            assert_eq!(s2.status, "error");
            assert!(s2.error.as_ref().unwrap().contains("Target file already exists"));
        } else {
            assert_eq!(s1.status, "error");
            assert!(s1.error.as_ref().unwrap().contains("Target file already exists"));
            assert_eq!(s2.status, "ok");
        }

        // Verify that target.flac exists and only one of the original source files was renamed to it
        assert!(temp_dir.join("target.flac").exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

