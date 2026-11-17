//! WCAG contrast of the theme's ink on the Luminous field and its glass
//! (docs/ui.md, "Luminous": text keeps AA on the stronger field; ink on
//! glass never drops below `ink-70` for secondary text). Colours are the
//! core's sRGB theme colours; translucent ones are composited over what
//! lies under them, as the screen draws them.

/// An sRGB colour with alpha, 0–255 per channel (the core's `Theme`).
pub type Rgba = [u8; 4];

/// `top` drawn over the opaque `bottom` (source-over, in sRGB as GTK
/// composites without a linear blend).
pub fn over(top: Rgba, bottom: Rgba) -> Rgba {
    let a = f64::from(top[3]) / 255.0;
    let mut out = [0u8; 4];
    for i in 0..3 {
        out[i] = (f64::from(top[i]) * a + f64::from(bottom[i]) * (1.0 - a)).round() as u8;
    }
    out[3] = 255;
    out
}

/// `colour` with its alpha multiplied by `k` (a glow at some strength).
pub fn fade(colour: Rgba, k: f64) -> Rgba {
    let a = (f64::from(colour[3]) * k.clamp(0.0, 1.0)).round() as u8;
    [colour[0], colour[1], colour[2], a]
}

/// WCAG 2 relative luminance of an opaque sRGB colour.
pub fn luminance(c: Rgba) -> f64 {
    let channel = |v: u8| {
        let s = f64::from(v) / 255.0;
        if s <= 0.040_45 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(c[0]) + 0.7152 * channel(c[1]) + 0.0722 * channel(c[2])
}

/// WCAG 2 contrast ratio of two opaque colours (1 to 21).
pub fn ratio(a: Rgba, b: Rgba) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// The contrast of text `ink` drawn on `surface`, both possibly
/// translucent, over the opaque `ground`.
pub fn text_contrast(ink: Rgba, surface: Rgba, ground: Rgba) -> f64 {
    let back = over(surface, ground);
    ratio(over(ink, back), back)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::theme::Colors;

    /// AA for body text.
    const AA: f64 = 4.5;
    /// AA for large text (18.66 px bold, 24 px regular): section headings.
    const AA_LARGE: f64 = 3.0;

    /// A solid 8 × 8 PNG of `rgb` (stored, uncompressed), for the core's
    /// artwork theme.
    fn png(rgb: [u8; 3]) -> Vec<u8> {
        fn crc32(bytes: &[u8]) -> u32 {
            let mut crc = 0xffff_ffffu32;
            for &b in bytes {
                crc ^= u32::from(b);
                for _ in 0..8 {
                    crc = if crc & 1 == 1 {
                        (crc >> 1) ^ 0xedb8_8320
                    } else {
                        crc >> 1
                    };
                }
            }
            !crc
        }
        fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            let start = out.len();
            out.extend_from_slice(kind);
            out.extend_from_slice(data);
            let crc = crc32(&out[start..]);
            out.extend_from_slice(&crc.to_be_bytes());
        }
        let side = 8u32;
        // Each row: filter type 0, then the pixels.
        let row: Vec<u8> = std::iter::once(0)
            .chain(rgb.iter().copied().cycle().take(3 * side as usize))
            .collect();
        let raw = row.repeat(side as usize);
        let (mut a, mut b) = (1u32, 0u32);
        for &x in &raw {
            a = (a + u32::from(x)) % 65_521;
            b = (b + a) % 65_521;
        }
        let mut zlib = vec![0x78, 0x01, 0x01];
        zlib.extend_from_slice(&(raw.len() as u16).to_le_bytes());
        zlib.extend_from_slice(&(!(raw.len() as u16)).to_le_bytes());
        zlib.extend_from_slice(&raw);
        zlib.extend_from_slice(&((b << 16) | a).to_be_bytes());
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&side.to_be_bytes());
        ihdr.extend_from_slice(&side.to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        let mut out = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        chunk(&mut out, b"IHDR", &ihdr);
        chunk(&mut out, b"IDAT", &zlib);
        chunk(&mut out, b"IEND", &[]);
        out
    }

    /// The themes the core makes for covers around the hue circle (and for
    /// no cover), light and dark, with the default Transparency.
    fn themes() -> Vec<(String, Colors)> {
        let covers: [Option<[u8; 3]>; 9] = [
            None,
            Some([220, 40, 40]),
            Some([230, 140, 20]),
            Some([230, 210, 30]),
            Some([40, 180, 60]),
            Some([30, 190, 200]),
            Some([40, 90, 220]),
            Some([140, 60, 210]),
            Some([220, 50, 160]),
        ];
        let mut out = Vec::new();
        for cover in covers {
            let bytes = cover.map(png);
            let t = jf_core::theme::theme_for_artwork(bytes.as_deref());
            assert!(cover.is_none() || t.pick.from_artwork, "{cover:?} gave no hue");
            for (dark, theme) in [(false, &t.light), (true, &t.dark)] {
                let name = format!("hue {:.0} {}", t.pick.hue, if dark { "dark" } else { "light" });
                out.push((name, theme.with_transparency(0.5).colors));
            }
        }
        out
    }

    /// The strongest place of the Luminous field: the sky under the first
    /// glow at its centre (`field.aurora-strength` × 1.07, as the
    /// stylesheet draws it).
    fn strongest_field(c: &Colors) -> Rgba {
        let strength = f64::from(jf_tokens::field::AURORA_STRENGTH) * 1.07;
        over(fade(c["field.aurora-1"], strength), c["field.sky"])
    }

    #[test]
    fn composites_and_measures_like_wcag() {
        assert_eq!(over([0, 0, 0, 255], [255, 255, 255, 255]), [0, 0, 0, 255]);
        assert_eq!(over([0, 0, 0, 0], [10, 20, 30, 255]), [10, 20, 30, 255]);
        assert!((ratio([0, 0, 0, 255], [255, 255, 255, 255]) - 21.0).abs() < 1e-9);
        assert!((ratio([118, 118, 118, 255], [255, 255, 255, 255]) - 4.54).abs() < 0.01);
    }

    #[test]
    fn text_keeps_aa_on_glass_over_the_strongest_field() {
        // Covers around the hue circle, light and dark: secondary text
        // (ink-70, the floor on glass), primary text and accent text on the
        // Mist groups, the Clear sidebar, the Veil capsules and solid Cloud,
        // at the field's strongest point.
        for (name, c) in themes() {
            let ground = strongest_field(&c);
            for surface in [
                "glass.tint-mist",
                "glass.tint-clear",
                "glass.tint-veil",
                "glass.cloud",
            ] {
                for ink in ["ink.ink-70", "ink.ink-90", "accent.accent-text"] {
                    let k = text_contrast(c[ink], c[surface], ground);
                    assert!(k >= AA, "{ink} on {surface}, {name}: {k:.2}");
                }
            }
        }
    }

    #[test]
    fn page_text_keeps_aa_on_the_strongest_field() {
        // Straight on the field (no glass): text at ink-70 and ink-90 keeps
        // AA; accent text there is for large headings (the sources' names
        // in Search), which keep AA for large text. ink-50 would not hold
        // (about 3.8:1 at worst), so secondary text uses ink-70.
        for (name, c) in themes() {
            let ground = strongest_field(&c);
            for ink in ["ink.ink-70", "ink.ink-90"] {
                let k = ratio(over(c[ink], ground), ground);
                assert!(k >= AA, "{ink} on the field, {name}: {k:.2}");
            }
            let k = ratio(over(c["accent.accent-text"], ground), ground);
            assert!(k >= AA_LARGE, "accent text on the field, {name}: {k:.2}");
        }
    }
}
