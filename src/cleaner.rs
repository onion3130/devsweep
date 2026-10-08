use crate::scanner::TrashDir;

/// Delete all `hits` dirs, returning total bytes freed (based on pre-scan sizes).
/// Failures are reported to stderr but don't stop the sweep.
pub fn remove_all(hits: &[TrashDir]) -> u64 {
    let mut freed = 0u64;
    for h in hits {
        match std::fs::remove_dir_all(&h.path) {
            Ok(()) => freed += h.size_bytes,
            Err(e) => eprintln!("failed to remove {}: {e}", h.path.display()),
        }
    }
    freed
}
