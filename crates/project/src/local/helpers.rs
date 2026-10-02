use std::{fs, io, path::Path};

/// Writes `target` whole, through a temporary file beside it, so a reader never sees half of it. The
/// file keeps its permissions, so saving a script keeps it runnable.
pub(crate) fn write_whole(target: &Path, bytes: &[u8]) -> io::Result<()> {
    let name = target.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let temporary = target.with_file_name(format!(".{name}.atelier-save"));
    fs::write(&temporary, bytes)?;
    if let Ok(meta) = fs::metadata(target) {
        fs::set_permissions(&temporary, meta.permissions())?;
    }
    fs::rename(&temporary, target).inspect_err(|_| {
        let _ = fs::remove_file(&temporary);
    })
}
