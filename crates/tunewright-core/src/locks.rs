use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Condvar, Mutex, OnceLock};

struct PathLockRegistry {
    locked_paths: Mutex<HashSet<PathBuf>>,
    condvar: Condvar,
}

static REGISTRY: OnceLock<PathLockRegistry> = OnceLock::new();

fn get_registry() -> &'static PathLockRegistry {
    REGISTRY.get_or_init(|| PathLockRegistry {
        locked_paths: Mutex::new(HashSet::new()),
        condvar: Condvar::new(),
    })
}

pub struct FileLockGuard {
    path: PathBuf,
    /// Held open purely to retain the OS-level advisory lock; released on drop.
    _os_lock: Option<std::fs::File>,
}

impl Drop for FileLockGuard {
    fn drop(&mut self) {
        let registry = get_registry();
        let mut locked = registry.locked_paths.lock().unwrap();
        locked.remove(&self.path);
        registry.condvar.notify_all();
    }
}

/// Total time spent trying to take the OS lock before giving up and relying on
/// the in-process lock alone. `File::lock` blocks indefinitely, which would let
/// any local process wedge a rayon worker permanently, so it is never used.
const OS_LOCK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

fn try_lock_with_deadline(file: std::fs::File) -> Option<std::fs::File> {
    let deadline = std::time::Instant::now() + OS_LOCK_TIMEOUT;
    let mut backoff = std::time::Duration::from_millis(1);
    loop {
        match file.try_lock() {
            Ok(()) => return Some(file),
            Err(_) if std::time::Instant::now() < deadline => {
                std::thread::sleep(backoff);
                backoff = (backoff * 2).min(std::time::Duration::from_millis(100));
            }
            Err(_) => {
                tracing::warn!("timed out waiting for an OS lock; proceeding without it");
                return None;
            }
        }
    }
}

/// Acquire an exclusive lock for `path`, serializing both threads in this
/// process and other processes sharing the same data directory.
///
/// The OS lock is taken on a read-only descriptor, so it never truncates or
/// creates the target. It is best-effort: if the file cannot be opened or the
/// filesystem does not support advisory locking (some network mounts), the
/// in-process lock still applies.
pub fn lock_file(path: &Path) -> FileLockGuard {
    let canonical_path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

    let registry = get_registry();
    let mut locked = registry.locked_paths.lock().unwrap();
    while locked.contains(&canonical_path) {
        locked = registry.condvar.wait(locked).unwrap();
    }
    locked.insert(canonical_path.clone());
    drop(locked);

    let os_lock = std::fs::File::open(&canonical_path)
        .ok()
        .and_then(try_lock_with_deadline);

    FileLockGuard {
        path: canonical_path,
        _os_lock: os_lock,
    }
}

/// Acquire process-global locks for two file paths safely to prevent deadlocks.
pub fn lock_two_files(p1: &Path, p2: &Path) -> (FileLockGuard, FileLockGuard) {
    let cp1 = std::fs::canonicalize(p1).unwrap_or_else(|_| p1.to_path_buf());
    let cp2 = std::fs::canonicalize(p2).unwrap_or_else(|_| p2.to_path_buf());

    if cp1 == cp2 {
        let g1 = lock_file(&cp1);
        let g2 = FileLockGuard {
            path: PathBuf::new(),
            _os_lock: None,
        };
        (g1, g2)
    } else if cp1 < cp2 {
        let g1 = lock_file(&cp1);
        let g2 = lock_file(&cp2);
        (g1, g2)
    } else {
        let g2 = lock_file(&cp2);
        let g1 = lock_file(&cp1);
        (g1, g2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_concurrent_locks() {
        let path = Path::new("some_file.mp3");
        let counter = Arc::new(AtomicUsize::new(0));

        let mut handles = vec![];
        for _ in 0..5 {
            let counter = counter.clone();
            handles.push(thread::spawn(move || {
                let _lock = lock_file(path);
                // We are inside the lock.
                // Increment counter to show we entered
                let val = counter.fetch_add(1, Ordering::SeqCst);
                // Sleep to allow other threads to potentially enter if lock wasn't working
                thread::sleep(Duration::from_millis(50));
                // Verify that while we were inside, the counter value didn't change (no other thread entered)
                assert_eq!(counter.load(Ordering::SeqCst), val + 1);
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(counter.load(Ordering::SeqCst), 5);
    }
    #[test]
    fn os_lock_excludes_other_processes() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("tw_lock_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("song.mp3");
        std::fs::File::create(&target)
            .unwrap()
            .write_all(b"x")
            .unwrap();

        let guard = lock_file(&target);

        // A second descriptor stands in for another process; try_lock must fail
        // while the guard is alive, and succeed once it is dropped.
        let probe = std::fs::File::open(&target).unwrap();
        assert!(probe.try_lock().is_err(), "lock should be held");
        drop(probe);

        drop(guard);

        let probe2 = std::fs::File::open(&target).unwrap();
        assert!(probe2.try_lock().is_ok(), "lock should be free after drop");

        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn os_lock_gives_up_instead_of_blocking_forever() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("tw_lock_to_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("held.mp3");
        std::fs::File::create(&target)
            .unwrap()
            .write_all(b"x")
            .unwrap();

        // A separate descriptor holds the lock for the whole test.
        let holder = std::fs::File::open(&target).unwrap();
        holder.lock().unwrap();

        let start = std::time::Instant::now();
        let guard = lock_file(&target);
        let waited = start.elapsed();

        assert!(
            waited >= OS_LOCK_TIMEOUT,
            "should have waited out the deadline, waited {waited:?}"
        );
        assert!(
            waited < OS_LOCK_TIMEOUT * 3,
            "must not block indefinitely, waited {waited:?}"
        );

        drop(guard);
        drop(holder);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
