//! QR codes for sign-in steps (`show-qr`): the module matrix as one SVG
//! path, which the page draws in its ink colour (the Linux app draws the
//! same matrix with GTK).

use qrcode::{Color, QrCode};

/// An SVG path (`d` attribute) of the dark modules, one unit per module,
/// and the matrix width, or `None` when the data does not fit a QR code.
pub fn svg_path(data: &str) -> Option<(String, usize)> {
    let code = QrCode::new(data.as_bytes()).ok()?;
    let width = code.width();
    let colors = code.to_colors();
    let mut path = String::with_capacity(width * width * 4);
    for (index, color) in colors.iter().enumerate() {
        if *color == Color::Dark {
            let (x, y) = (index % width, index / width);
            path.push_str(&format!("M{x} {y}h1v1h-1z"));
        }
    }
    Some((path, width))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_a_matrix() {
        let (path, width) = svg_path("https://music.example.com/qr/123").unwrap();
        assert!(width >= 21 && width % 4 == 1, "{width}");
        assert!(path.starts_with("M0 0h1v1h-1z"), "finder pattern corner");
        assert!(svg_path(&"x".repeat(5000)).is_none());
    }
}
