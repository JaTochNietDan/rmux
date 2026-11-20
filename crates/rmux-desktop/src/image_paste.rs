//! Bounded image files retained by the terminal that pasted their paths.
use anyhow::{Result, ensure};
use gpui::{Image, ImageFormat};
use std::{io::Write, sync::Mutex, path::{Path, PathBuf}, fs::{File, OpenOptions}, os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt}};
static USAGE: Mutex<(usize, usize)> = Mutex::new((0, 0));
const MAX_IMAGE: usize = 16 * 1024 * 1024;
const MAX_TOTAL: usize = 64 * 1024 * 1024;
const MAX_FILES: usize = 32;

fn storage_root() -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    let base = std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Caches"));
    #[cfg(not(target_os = "macos"))]
    let base = std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")));
    let base = base.ok_or_else(|| anyhow::anyhow!("No user cache directory for image paste"))?;
    ensure!(base.is_absolute(), "Image cache directory must be absolute");
    Ok(base.join("rmux/pasted-images"))
}

/// Serializes publication and cleanup between rmux processes. The lock file is
/// never unlinked, so a second process cannot lock a replacement inode.
fn lock_storage(root: &Path) -> Result<File> {
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(root)?;
    let metadata = std::fs::symlink_metadata(root)?;
    ensure!(metadata.is_dir() && !metadata.file_type().is_symlink() && metadata.permissions().mode() & 0o077 == 0,
        "Image cache must be a private directory, not a symlink");
    let path = root.join(".lock");
    if let Ok(metadata) = std::fs::symlink_metadata(&path) {
        ensure!(metadata.is_file() && !metadata.file_type().is_symlink(), "Invalid image cache lock");
    }
    let lock = OpenOptions::new().read(true).write(true).create(true).truncate(false).mode(0o600).open(path)?;
    lock.try_lock().map_err(|error| anyhow::anyhow!("Image cache is busy; retry paste: {error}"))?;
    Ok(lock)
}

fn cleanup_unlocked(root: &Path) -> Result<usize> {
    let mut removed = 0;
    // Bounded per paste, with no filesystem walk outside this private directory.
    for entry in std::fs::read_dir(root)?.take(512) {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue; };
        if !name.starts_with("rmux-image-") || !entry.file_type()?.is_file() { continue; }
        let file = match OpenOptions::new().read(true).write(true).open(entry.path()) {
            Ok(file) => file, Err(_) => continue,
        };
        if file.try_lock().is_ok() && std::fs::remove_file(entry.path()).is_ok() { removed += 1; }
    }
    Ok(removed)
}

pub struct Reservation(usize);
impl Reservation {
    pub fn acquire(bytes: usize) -> Result<Self> {
        ensure!(bytes > 0 && bytes <= MAX_IMAGE, "Clipboard image must be between 1 byte and 16 MiB");
        let mut usage = USAGE.lock().unwrap_or_else(|error| error.into_inner());
        ensure!(usage.0 < MAX_FILES && bytes <= MAX_TOTAL - usage.1,
            "Image paste storage is full (32 files / 64 MiB); close terminals retaining images first");
        usage.0 += 1;
        usage.1 += bytes;
        Ok(Self(bytes))
    }
}
impl Drop for Reservation {
    fn drop(&mut self) {
        let mut usage = USAGE.lock().unwrap_or_else(|error| error.into_inner());
        usage.0 -= 1;
        usage.1 -= self.0;
    }
}
pub struct Prepared {
    file: tempfile::NamedTempFile,
    _reservation: Reservation,
}
impl Prepared {
    pub fn write(image: Image, reservation: Reservation) -> Result<Self> {
        Self::write_in(image, reservation, &storage_root()?)
    }
    fn write_in(image: Image, reservation: Reservation, root: &Path) -> Result<Self> {
        ensure!(image.bytes.len() == reservation.0, "Image reservation does not match clipboard data");
        let suffix = match image.format {
            ImageFormat::Png => ".png", ImageFormat::Jpeg => ".jpg", ImageFormat::Webp => ".webp",
            ImageFormat::Gif => ".gif", ImageFormat::Svg => ".svg", ImageFormat::Bmp => ".bmp", ImageFormat::Tiff => ".tiff",
        };
        let publication = lock_storage(root)?;
        cleanup_unlocked(root)?;
        let mut file = tempfile::Builder::new().prefix("rmux-image-").suffix(suffix).tempfile_in(root)?;
        file.as_file().lock()?;
        drop(publication);
        file.write_all(&image.bytes)?;
        file.flush()?;
        Ok(Self { file, _reservation: reservation })
    }
    pub fn path(&self) -> &std::path::Path { self.file.path() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn files_preserve_bytes_permissions_and_release_storage() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        assert!(Reservation::acquire(0).is_err());
        assert!(Reservation::acquire(MAX_IMAGE + 1).is_err());
        let bytes = b"image fixture".to_vec();
        let directory = tempfile::tempdir()?;
        let root = directory.path().join("images");
        let prepared = Prepared::write_in(Image::from_bytes(ImageFormat::Png, bytes.clone()), Reservation::acquire(bytes.len())?, &root)?;
        let path = prepared.path().to_owned();
        assert_eq!(std::fs::read(&path)?, bytes);
        assert_eq!(std::fs::metadata(&path)?.permissions().mode() & 0o777, 0o600);
        assert_eq!(path.extension().unwrap(), "png");
        let publication = lock_storage(&root)?;
        assert!(lock_storage(&root).is_err(), "publication cannot race cleanup");
        assert_eq!(cleanup_unlocked(&root)?, 0, "live image must remain locked");
        let orphan = root.join("rmux-image-orphan.png");
        std::fs::write(&orphan, b"abandoned")?;
        let unrelated = root.join("unrelated.txt");
        std::fs::write(&unrelated, b"keep")?;
        let link = root.join("rmux-image-link.png");
        std::os::unix::fs::symlink(&unrelated, &link)?;
        assert_eq!(cleanup_unlocked(&root)?, 1);
        assert!(!orphan.exists());
        assert!(unrelated.exists() && link.exists() && path.exists());
        drop(publication);
        drop(prepared);
        assert!(!path.exists());
        let reservations: Vec<_> = (0..4).map(|_| Reservation::acquire(MAX_IMAGE)).collect::<Result<_>>()?;
        assert!(Reservation::acquire(1).is_err());
        drop(reservations);
        let reservations: Vec<_> = (0..MAX_FILES).map(|_| Reservation::acquire(1)).collect::<Result<_>>()?;
        assert!(Reservation::acquire(1).is_err());
        drop(reservations);
        assert!(Reservation::acquire(MAX_IMAGE).is_ok());
        Ok(())
    }
}
