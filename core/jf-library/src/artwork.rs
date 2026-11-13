//! Content-addressed artwork cache: `<cache_dir>/artwork/<blake3 hex>.<ext>`.
//!
//! The same image embedded in every track of an album is stored once.

use std::fs;
use std::io;
use std::path::Path;

use lofty::picture::MimeType;

/// An image in the cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredArt {
    pub hash: String,
    pub file_name: String,
    pub mime: Option<String>,
    pub byte_len: u64,
}

/// Writes `data` to `dir` unless an image with the same hash is already there.
pub(crate) fn store(dir: &Path, data: &[u8], mime: Option<&MimeType>) -> io::Result<StoredArt> {
    let hash = blake3::hash(data).to_hex().to_string();
    let sniffed = sniff(data);
    let ext = sniffed
        .map(|(ext, _)| ext)
        .or_else(|| mime.and_then(MimeType::ext))
        .unwrap_or("img");
    let file_name = format!("{hash}.{ext}");
    let path = dir.join(&file_name);
    if !path.is_file() {
        fs::create_dir_all(dir)?;
        // Write then rename, so a reader never sees half an image.
        let partial = dir.join(format!(".{file_name}.{}.part", std::process::id()));
        fs::write(&partial, data)?;
        if let Err(err) = fs::rename(&partial, &path) {
            let _ = fs::remove_file(&partial);
            if !path.is_file() {
                return Err(err);
            }
        }
    }
    Ok(StoredArt {
        hash,
        file_name,
        mime: sniffed
            .map(|(_, mime)| mime.to_owned())
            .or_else(|| mime.map(|m| m.as_str().to_owned())),
        byte_len: data.len() as u64,
    })
}

/// Recognises an image by its magic bytes: (extension, MIME type).
fn sniff(data: &[u8]) -> Option<(&'static str, &'static str)> {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(("png", "image/png"))
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(("jpg", "image/jpeg"))
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        Some(("gif", "image/gif"))
    } else if data.len() >= 12 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP" {
        Some(("webp", "image/webp"))
    } else if data.starts_with(b"BM") {
        Some(("bmp", "image/bmp"))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_each_image_once() {
        let dir = tempfile::tempdir().unwrap();
        let jpeg = [0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3];
        let first = store(dir.path(), &jpeg, Some(&MimeType::Png)).unwrap();
        assert_eq!(
            first.file_name,
            format!("{}.jpg", first.hash),
            "content wins over the tag"
        );
        assert_eq!(first.mime.as_deref(), Some("image/jpeg"));
        let again = store(dir.path(), &jpeg, None).unwrap();
        assert_eq!(again, first);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        assert_eq!(fs::read(dir.path().join(&first.file_name)).unwrap(), jpeg);

        let unknown = store(dir.path(), b"????", None).unwrap();
        assert!(unknown.file_name.ends_with(".img"));
        assert_eq!(unknown.mime, None);
    }
}
