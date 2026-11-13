//! Artwork thumbnails: sizes, aspect, reuse, purged caches and cleanup.

use std::fs;
use std::path::Path;

use jf_library::LibraryError;

use crate::fixtures::{self, Fixture};

/// Writes a tagged FLAC file with `cover` as its front cover.
fn with_cover(fx: &Fixture, file: &str, cover: Vec<u8>) {
    let path = fx.path(file);
    fixtures::flac(&path, 50);
    fixtures::tag(&path, |t| {
        fixtures::basic(t, file, "Painter", file, 1);
        t.push_picture(fixtures::front_cover(cover));
    });
}

fn dimensions(path: &Path) -> (u32, u32) {
    let image = image::open(path).unwrap();
    (image.width(), image.height())
}

#[test]
fn thumbnails_fit_their_box_keep_the_aspect_and_are_reused() {
    let fx = Fixture::new();
    with_cover(&fx, "wide.flac", fixtures::png(300, 150, [200, 30, 30]));
    with_cover(&fx, "tall.flac", fixtures::png(100, 400, [30, 30, 200]));
    fx.scan();
    let wide = fx.track_at("wide.flac").artwork_hash.unwrap();
    let tall = fx.track_at("tall.flac").artwork_hash.unwrap();
    let revision = fx.lib.revision().unwrap();

    let thumb = fx.lib.artwork_thumbnail(&wide, 100).unwrap().unwrap();
    assert_eq!(thumb, fx.artwork_dir().join(&wide).join("128.jpg"));
    let bytes = fs::read(&thumb).unwrap();
    assert!(bytes.starts_with(&[0xFF, 0xD8, 0xFF]), "a JPEG");
    let pixel = image::load_from_memory(&bytes)
        .unwrap()
        .to_rgb8()
        .get_pixel(64, 32)
        .0;
    let close = |got: u8, want: u8| got.abs_diff(want) <= 8;
    assert!(
        close(pixel[0], 200) && close(pixel[1], 30) && close(pixel[2], 30),
        "{pixel:?}"
    );

    for (asked, bucket, size) in [
        (0, 64, (64, 32)),
        (64, 64, (64, 32)),
        (100, 128, (128, 64)),
        (129, 256, (256, 128)),
        (5000, 1024, (300, 150)),
    ] {
        let path = fx.lib.artwork_thumbnail(&wide, asked).unwrap().unwrap();
        assert_eq!(path, fx.artwork_dir().join(&wide).join(format!("{bucket}.jpg")));
        assert_eq!(dimensions(&path), size, "{asked} px, never enlarged");
    }
    for (asked, size) in [(64, (16, 64)), (256, (64, 256)), (512, (100, 400))] {
        let path = fx.lib.artwork_thumbnail(&tall, asked).unwrap().unwrap();
        assert_eq!(dimensions(&path), size, "portrait {asked} px");
    }
    assert_eq!(
        fx.lib.revision().unwrap(),
        revision,
        "thumbnails are no catalogue change"
    );

    // Made once and reused.
    fs::write(&thumb, b"kept").unwrap();
    assert_eq!(fx.lib.artwork_thumbnail(&wide, 128).unwrap(), Some(thumb.clone()));
    assert_eq!(fs::read(&thumb).unwrap(), b"kept");
    let leftovers: Vec<_> = fs::read_dir(fx.artwork_dir().join(&wide))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| !name.ends_with(".jpg"))
        .collect();
    assert!(leftovers.is_empty(), "no temporary files left: {leftovers:?}");

    // A purged cache is rebuilt from the audio file.
    fs::remove_dir_all(fx.artwork_dir()).unwrap();
    let rebuilt = fx.lib.artwork_thumbnail(&wide, 64).unwrap().unwrap();
    assert_eq!(dimensions(&rebuilt), (64, 32));
    assert!(fx.artwork_dir().join(format!("{wide}.png")).is_file());

    for hash in [
        "0".repeat(64),
        wide.to_uppercase(),
        "../../etc/passwd".to_owned(),
        String::new(),
    ] {
        assert_eq!(fx.lib.artwork_thumbnail(&hash, 64).unwrap(), None, "{hash:?}");
    }

    // Thumbnails go with their artwork.
    fs::remove_file(fx.path("wide.flac")).unwrap();
    assert_eq!(fx.scan().removed, 1);
    assert!(!fx.artwork_dir().join(&wide).exists());
    assert_eq!(fx.lib.artwork_thumbnail(&wide, 64).unwrap(), None);
    assert!(fx.lib.artwork_thumbnail(&tall, 64).unwrap().is_some());
}

#[test]
fn a_damaged_cover_fails_with_an_artwork_error() {
    let fx = Fixture::new();
    let mut damaged = b"\x89PNG\r\n\x1a\n".to_vec();
    damaged.extend([7; 40]);
    with_cover(&fx, "damaged.flac", damaged);
    fx.scan();
    let hash = fx.track_at("damaged.flac").artwork_hash.unwrap();

    let err = fx.lib.artwork_thumbnail(&hash, 64).unwrap_err();
    assert!(
        matches!(&err, LibraryError::Artwork { hash: h, .. } if *h == hash),
        "{err}"
    );
    assert!(!fx.artwork_dir().join(&hash).join("64.jpg").exists());
}
