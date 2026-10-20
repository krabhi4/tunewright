use std::io::Write;
use std::path::Path;

pub fn restrict_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)) {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!("Could not restrict permissions on {:?}: {}", path, e);
            }
        }
    }
    #[cfg(not(unix))]
    let _ = path;
}

pub fn write_private_atomically(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let tmp_path = path.with_extension("json.tmp");
    let _ = std::fs::remove_file(&tmp_path);

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&tmp_path)?;
    file.write_all(contents)?;
    file.sync_all()?;
    drop(file);

    std::fs::rename(&tmp_path, path)?;

    if let Some(parent) = path.parent() {
        if let Ok(dir) = std::fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }
    Ok(())
}
