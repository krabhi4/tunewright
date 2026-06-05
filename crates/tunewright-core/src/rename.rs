use crate::audio;
use crate::format_string;
use crate::types::TunewrightError;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenamePreview {
    pub id: String,
    pub old_name: String,
    pub new_name: String,
    pub conflict: bool,
}

/// Preview renames without executing
pub fn preview_renames(
    data_root: &Path,
    files: &[(String, String)], // (id, relative_path)
    format: &str,
) -> Result<Vec<RenamePreview>, TunewrightError> {
    // Reads are independent, so compute (id, old_name, new_name) in parallel.
    // Conflict detection is order-dependent, so it runs as a sequential pass below.
    let computed: Vec<(String, String, String)> = files
        .par_iter()
        .map(|(id, rel_path)| {
            let full_path = data_root.join(rel_path);
            let old_name = full_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let extension = full_path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let tags = audio::read_tags_fast(&full_path).unwrap_or_default();
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
    files: &[(String, String)], // (id, relative_path)
    format: &str,
) -> Vec<RenameResult> {
    let previews = match preview_renames(data_root, files, format) {
        Ok(p) => p,
        Err(e) => {
            return files
                .iter()
                .map(|(id, rel_path)| RenameResult {
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
        .map(|(preview, (_, rel_path))| {
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

            let old_path = data_root.join(rel_path);
            let new_path = old_path.with_file_name(&preview.new_name);

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
                    return RenameResult {
                        id: preview.id,
                        status: "error".to_string(),
                        old_name: preview.old_name,
                        new_name: preview.new_name,
                        new_relative_path: unchanged_rel,
                        error: Some("Target file already exists".to_string()),
                    };
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
        let files = vec![("1".to_string(), old_rel.to_string())];
        let results = execute_renames(&temp_dir, &files, "target");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, "error");
        assert!(results[0].error.as_ref().unwrap().contains("Target file already exists"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    fn rand_num() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    }
}
