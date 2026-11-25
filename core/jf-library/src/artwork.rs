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

/// Names of cover pictures kept next to the music, most likely first.
const FOLDER_ART: [&str; 5] = ["cover", "folder", "front", "album", "artwork"];
/// Their extensions.
const FOLDER_ART_EXTS: [&str; 5] = ["jpg", "jpeg", "png", "webp", "gif"];
/// Largest folder picture read.
const MAX_FOLDER_ART: u64 = 32 * 1024 * 1024;

/// The cover picture among `files` (a folder's files): `cover.jpg`,
/// `folder.png`, `front.webp`, … in that order of names, any case.
pub fn folder_art_among(files: &[std::path::PathBuf]) -> Option<std::path::PathBuf> {
    files
        .iter()
        .filter_map(|path| {
            let stem = path.file_stem()?.to_str()?.to_ascii_lowercase();
            let ext = path.extension()?.to_str()?.to_ascii_lowercase();
            let rank = FOLDER_ART.iter().position(|n| *n == stem)?;
            let ext_rank = FOLDER_ART_EXTS.iter().position(|e| *e == ext)?;
            Some(((rank, ext_rank), path))
        })
        .min_by_key(|(rank, _)| *rank)
        .map(|(_, path)| path.clone())
}

/// The bytes of a folder picture (at most [`MAX_FOLDER_ART`]); an empty
/// file is no picture.
pub(crate) fn read_folder_art(fs: &dyn crate::fs::FileSystem, picture: &Path) -> io::Result<Vec<u8>> {
    use std::io::Read;
    let mut bytes = Vec::new();
    fs.open(picture)?.take(MAX_FOLDER_ART).read_to_end(&mut bytes)?;
    if bytes.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "the picture is empty"));
    }
    Ok(bytes)
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

    #[test]
    fn finds_folder_art_by_name_then_extension() {
        let files = |names: &[&str]| -> Vec<std::path::PathBuf> {
            names.iter().map(|n| std::path::PathBuf::from("/m").join(n)).collect()
        };
        assert_eq!(folder_art_among(&files(&["01.flac", "notes.txt"])), None);
        assert_eq!(
            folder_art_among(&files(&["Front.PNG", "01.flac"])),
            Some("/m/Front.PNG".into())
        );
        assert_eq!(
            folder_art_among(&files(&["front.png", "Cover.webp", "cover.gif"])),
            Some("/m/Cover.webp".into()),
            "cover before front; webp before gif"
        );
        assert_eq!(folder_art_among(&files(&["cover.bmp"])), None);
    }
}
