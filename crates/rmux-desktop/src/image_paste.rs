//! Bounded image files retained by the terminal that pasted their paths.
use anyhow::{Result, ensure};
use gpui::{Image, ImageFormat};
use std::{io::Write, sync::Mutex};
static USAGE: Mutex<(usize, usize)> = Mutex::new((0, 0));
const MAX_IMAGE: usize = 16 * 1024 * 1024;
const MAX_TOTAL: usize = 64 * 1024 * 1024;
const MAX_FILES: usize = 32;

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
        ensure!(image.bytes.len() == reservation.0, "Image reservation does not match clipboard data");
        let suffix = match image.format {
            ImageFormat::Png => ".png", ImageFormat::Jpeg => ".jpg", ImageFormat::Webp => ".webp",
            ImageFormat::Gif => ".gif", ImageFormat::Svg => ".svg", ImageFormat::Bmp => ".bmp", ImageFormat::Tiff => ".tiff",
        };
        let mut file = tempfile::Builder::new().prefix("rmux-image-").suffix(suffix).tempfile()?;
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
        let prepared = Prepared::write(Image::from_bytes(ImageFormat::Png, bytes.clone()), Reservation::acquire(bytes.len())?)?;
        let path = prepared.path().to_owned();
        assert_eq!(std::fs::read(&path)?, bytes);
        assert_eq!(std::fs::metadata(&path)?.permissions().mode() & 0o777, 0o600);
        assert_eq!(path.extension().unwrap(), "png");
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
