use crate::types::TunewrightError;
use sha2::{Digest, Sha256};
use std::path::Path;

/// Crash-safe in-place file mutation: run `mutate` against a temp copy of
/// `path` in the same directory, fsync the temp file, then atomically rename
/// it over the original (best-effort fsync of the parent directory after).
///
/// If `mutate` (or any step) fails, the original file is left untouched and
/// the temp copy is removed. A crash mid-`mutate` leaves the original intact;
/// a crash after the rename leaves the fully-written new file.
pub fn atomic_file_update<F>(path: &Path, mutate: F) -> Result<(), TunewrightError>
where
    F: FnOnce(&Path) -> Result<(), TunewrightError>,
{
    let file_name = path.file_name().ok_or_else(|| {
        TunewrightError::TagWriteError(format!("{}: invalid file name", path.display()))
    })?;
    // Process id keeps concurrent instances sharing a data directory from
    // clobbering each other's temp copy. The source name is hashed rather than
    // embedded so the result is always well under the 255-byte NAME_MAX.
    // The extension is preserved because lofty infers the container format
    // from it; only the stem is hashed.
    let mut hasher = Sha256::new();
    hasher.update(file_name.as_encoded_bytes());
    let digest = hasher.finalize();
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let tmp_path = path.with_file_name(format!(
        ".tw-tmp-{}-{}{}",
        std::process::id(),
        hex::encode(&digest[..8]),
        ext
    ));

    let result = (|| {
        // fs::copy preserves permissions, keeping the swapped-in file consistent.
        std::fs::copy(path, &tmp_path)
            .map_err(|e| TunewrightError::TagWriteError(format!("{}: {}", path.display(), e)))?;
        mutate(&tmp_path)?;
        // Flush the temp file's data before the rename so a crash right after
        // the rename cannot surface a truncated/empty file.
        std::fs::File::open(&tmp_path)
            .and_then(|f| f.sync_all())
            .map_err(|e| {
                TunewrightError::TagWriteError(format!("{}: {}", tmp_path.display(), e))
            })?;
        std::fs::rename(&tmp_path, path)
            .map_err(|e| TunewrightError::TagWriteError(format!("{}: {}", path.display(), e)))?;
        // Best-effort: persist the directory entry as well.
        if let Some(parent) = path.parent() {
            if let Ok(dir) = std::fs::File::open(parent) {
                let _ = dir.sync_all();
            }
        }
        Ok(())
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&tmp_path);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn leftover_temp_files(dir: &std::path::Path) -> Vec<String> {
        std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.starts_with(".tw-tmp-"))
            .collect()
    }

    #[test]
    fn test_atomic_update_success_replaces_content_and_cleans_temp() {
        let temp_dir = std::env::temp_dir().join(format!("tunewright_test_{}", rand_num()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let path = temp_dir.join("f.txt");
        std::fs::write(&path, b"old").unwrap();

        atomic_file_update(&path, |tmp| {
            std::fs::write(tmp, b"new").map_err(|e| TunewrightError::TagWriteError(e.to_string()))
        })
        .unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"new");
        assert_eq!(
            leftover_temp_files(&temp_dir),
            Vec::<String>::new(),
            "temp file must not be left behind on success"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_atomic_update_mutate_error_preserves_original() {
        let temp_dir = std::env::temp_dir().join(format!("tunewright_test_{}", rand_num()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let path = temp_dir.join("f.txt");
        std::fs::write(&path, b"old").unwrap();

        // Simulate a crash mid-write: partial bytes land in the temp copy,
        // then the mutation fails.
        let res = atomic_file_update(&path, |tmp| {
            std::fs::write(tmp, b"par").unwrap();
            Err(TunewrightError::TagWriteError("boom".to_string()))
        });

        assert!(res.is_err());
        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"old",
            "original must be byte-identical after a failed mutation"
        );
        assert_eq!(
            leftover_temp_files(&temp_dir),
            Vec::<String>::new(),
            "temp file must be cleaned up on failure"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_atomic_update_missing_source_errors() {
        let temp_dir = std::env::temp_dir().join(format!("tunewright_test_{}", rand_num()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let path = temp_dir.join("missing.txt");

        let res = atomic_file_update(&path, |_| Ok(()));
        assert!(res.is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
