use crate::audio;
use crate::format_string;
use crate::types::TagStudioError;
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
) -> Result<Vec<RenamePreview>, TagStudioError> {
    let mut previews = Vec::new();
    let mut used_names: HashSet<String> = HashSet::new();

    for (id, rel_path) in files {
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

        // Read tags for this file
        let tags = audio::read_tags_fast(&full_path).unwrap_or_default();
        let new_stem = format_string::evaluate(format, &tags);

        let new_name = if new_stem.is_empty() {
            old_name.clone()
        } else {
            format!("{}.{}", new_stem, extension)
        };

        let conflict = used_names.contains(&new_name.to_lowercase());
        used_names.insert(new_name.to_lowercase());

        previews.push(RenamePreview {
            id: id.clone(),
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
                .map(|(id, _)| RenameResult {
                    id: id.clone(),
                    status: "error".to_string(),
                    old_name: String::new(),
                    new_name: String::new(),
                    error: Some(e.to_string()),
                })
                .collect();
        }
    };

    previews
        .into_iter()
        .zip(files.iter())
        .map(|(preview, (_, rel_path))| {
            if preview.conflict {
                return RenameResult {
                    id: preview.id,
                    status: "error".to_string(),
                    old_name: preview.old_name,
                    new_name: preview.new_name,
                    error: Some("Duplicate filename conflict".to_string()),
                };
            }

            if preview.old_name == preview.new_name {
                return RenameResult {
                    id: preview.id,
                    status: "skipped".to_string(),
                    old_name: preview.old_name,
                    new_name: preview.new_name,
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
                            error: Some(format!("Failed to remove old file: {}", e)),
                        };
                    }
                    return RenameResult {
                        id: preview.id,
                        status: "ok".to_string(),
                        old_name: preview.old_name,
                        new_name: preview.new_name,
                        error: None,
                    };
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    return RenameResult {
                        id: preview.id,
                        status: "error".to_string(),
                        old_name: preview.old_name,
                        new_name: preview.new_name,
                        error: Some("Target file already exists".to_string()),
                    };
                }
                Err(_) => {
                    // hard_link fails across filesystems — fall back to rename
                }
            }

            // Fallback: standard rename (cross-filesystem or unsupported hard_link)
            match std::fs::rename(&old_path, &new_path) {
                Ok(()) => RenameResult {
                    id: preview.id,
                    status: "ok".to_string(),
                    old_name: preview.old_name,
                    new_name: preview.new_name,
                    error: None,
                },
                Err(e) => RenameResult {
                    id: preview.id,
                    status: "error".to_string(),
                    old_name: preview.old_name,
                    new_name: preview.new_name,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
