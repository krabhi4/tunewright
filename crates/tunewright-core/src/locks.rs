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
}

impl Drop for FileLockGuard {
    fn drop(&mut self) {
        let registry = get_registry();
        let mut locked = registry.locked_paths.lock().unwrap();
        locked.remove(&self.path);
        registry.condvar.notify_all();
    }
}

/// Acquire a process-global lock for the given file path to serialize writes.
pub fn lock_file(path: &Path) -> FileLockGuard {
    let canonical_path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

    let registry = get_registry();
    let mut locked = registry.locked_paths.lock().unwrap();
    while locked.contains(&canonical_path) {
        locked = registry.condvar.wait(locked).unwrap();
    }
    locked.insert(canonical_path.clone());

    FileLockGuard {
        path: canonical_path,
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
}
