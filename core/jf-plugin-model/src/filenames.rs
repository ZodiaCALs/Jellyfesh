//! Download file names (ADR-0025 §2): the person's template, and names that
//! are safe on every platform.
//!
//! The core names downloaded files with it, and Jellyfesh Web names the file
//! the browser saves, so both follow one set of rules:
//!
//! * a template is made of the fields `{artist}`, `{album_artist}`,
//!   `{album}`, `{title}`, `{track}`, `{disc}`, `{year}` and `{source}`;
//!   `/` (or `\`) separates folders, and a value never adds a folder of its
//!   own. The default is [`DEFAULT_TEMPLATE`];
//! * `<>:"/\|?*` and control characters become `_`; leading spaces and
//!   trailing dots and spaces go; a leading dot becomes `_` (no hidden
//!   files); names Windows reserves (`CON`, `PRN`, `AUX`, `NUL`, `COM0`–`COM9`,
//!   `LPT0`–`LPT9`, also with an extension) get a `_`; each part is at most
//!   [`MAX_PART`] bytes;
//! * a name that is taken gets " (2)", " (3)", … before its extension.
//!
//! The rules and their golden table are platform-neutral, so a library made
//! on Linux copies to a Windows or macOS drive unchanged.

/// The template when the person has not set one.
pub const DEFAULT_TEMPLATE: &str = "{album_artist}/{album}/{track} {title}";

/// The fields a template may use, in the order Settings lists them.
pub const FIELDS: [&str; 8] = [
    "artist",
    "album_artist",
    "album",
    "title",
    "track",
    "disc",
    "year",
    "source",
];

/// Longest part of a path (a folder or the file name), in bytes: the limit
/// of NTFS, APFS and ext4.
pub const MAX_PART: usize = 255;

/// What a downloaded song's name is made of.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NameFields {
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub album: Option<String>,
    pub title: String,
    pub track: Option<u32>,
    pub disc: Option<u32>,
    pub year: Option<u32>,
    /// The source's name ("Jellyfin").
    pub source: Option<String>,
}

impl NameFields {
    /// A field's text: `{track}` with two digits ("07"), the album artist
    /// falling back to the artist, missing names as "Unknown Artist",
    /// "Unknown Album" and "Unknown Title", missing numbers empty.
    fn value(&self, field: &str) -> Option<String> {
        fn name(value: &Option<String>) -> Option<&str> {
            value.as_deref().map(str::trim).filter(|v| !v.is_empty())
        }
        Some(match field {
            "artist" => name(&self.artist).unwrap_or("Unknown Artist").to_owned(),
            "album_artist" => name(&self.album_artist)
                .or_else(|| name(&self.artist))
                .unwrap_or("Unknown Artist")
                .to_owned(),
            "album" => name(&self.album).unwrap_or("Unknown Album").to_owned(),
            "title" => Some(self.title.trim())
                .filter(|t| !t.is_empty())
                .unwrap_or("Unknown Title")
                .to_owned(),
            "track" => self.track.map(|n| format!("{n:02}")).unwrap_or_default(),
            "disc" => self.disc.map(|n| n.to_string()).unwrap_or_default(),
            "year" => self.year.map(|n| n.to_string()).unwrap_or_default(),
            "source" => name(&self.source).unwrap_or_default().to_owned(),
            _ => return None,
        })
    }
}

/// The parts of the path a template gives for `fields`, relative to the
/// downloads folder: the folders, then the file's name without its
/// extension. Each part is safe ([`safe_part`]); parts left empty (a folder
/// of `{year}` for a song without one) are left out. Unknown fields stay as
/// written, so a typo shows in the preview. An empty template uses
/// [`DEFAULT_TEMPLATE`].
pub fn render(template: &str, fields: &NameFields) -> Vec<String> {
    let template = if template.trim().is_empty() {
        DEFAULT_TEMPLATE
    } else {
        template
    };
    let mut parts: Vec<String> = template
        .split(['/', '\\'])
        .map(|part| fill(part, fields))
        .filter(|part| !part.is_empty())
        .map(|part| safe_part(&part))
        .collect();
    if parts.is_empty() {
        parts.push(safe_part(&fields.value("title").unwrap_or_default()));
    }
    parts
}

/// One part of a template with its fields filled in, tidied: whitespace runs
/// become one space, and dashes left at either end by an empty field go
/// ("{track} - {title}" without a track is "Title").
fn fill(part: &str, fields: &NameFields) -> String {
    let mut out = String::new();
    let mut rest = part;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) => {
                let field = &after[..close];
                match fields.value(field) {
                    // A value never makes a folder: its own separators are replaced here.
                    Some(value) => out.push_str(&value.replace(['/', '\\'], "_")),
                    None => {
                        out.push('{');
                        out.push_str(field);
                        out.push('}');
                    }
                }
                rest = &after[close + 1..];
            }
            None => {
                out.push_str(&rest[open..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    let collapsed = out.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.trim_matches(|c: char| c == '-' || c == ' ').to_owned()
}

/// A folder or file name that every platform accepts: see the module's
/// rules. Never empty (`_` stands in for nothing).
pub fn safe_part(text: &str) -> String {
    let replaced: String = text
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let mut name = trim_ends(&replaced).to_owned();
    if let Some(rest) = name.strip_prefix('.') {
        name = format!("_{rest}");
    }
    if reserved(&name) {
        // Before the extension, if there is one: "CON_", "nul_.txt".
        name = match name.find('.') {
            Some(dot) => format!("{}_{}", &name[..dot], &name[dot..]),
            None => format!("{name}_"),
        };
    }
    name = cut(&name, MAX_PART);
    if name.is_empty() { "_".to_owned() } else { name }
}

/// Leading whitespace and trailing dots and whitespace removed.
fn trim_ends(text: &str) -> &str {
    text.trim_start()
        .trim_end_matches(|c: char| c == '.' || c.is_whitespace())
}

/// At most `max` bytes, cut at a character boundary, with the ends trimmed
/// again (a cut can leave a trailing space or dot).
fn cut(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    trim_ends(&text[..end]).to_owned()
}

/// Whether Windows reserves the name (`CON`, `PRN`, `AUX`, `NUL`, `COM0`–`COM9`,
/// `LPT0`–`LPT9` and the superscript `COM¹`–`LPT³`), with or without an
/// extension, in any case.
fn reserved(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or_default().trim_end();
    let upper = stem.to_uppercase();
    if matches!(
        upper.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) {
        return true;
    }
    let Some(number) = upper.strip_prefix("COM").or_else(|| upper.strip_prefix("LPT")) else {
        return false;
    };
    let mut chars = number.chars();
    matches!(
        (chars.next(), chars.next()),
        (Some('0'..='9' | '¹' | '²' | '³'), None)
    )
}

/// A file name from a stem and an extension (`flac`), safe and at most
/// [`MAX_PART`] bytes: the stem gives way when it is too long.
pub fn file_name(stem: &str, extension: &str) -> String {
    let extension: String = extension
        .trim()
        .trim_start_matches('.')
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(8)
        .collect::<String>()
        .to_ascii_lowercase();
    if extension.is_empty() {
        return safe_part(stem);
    }
    let stem = safe_part(stem);
    let room = MAX_PART - extension.len() - 1;
    let stem = cut(&stem, room);
    let stem = if stem.is_empty() { "_".to_owned() } else { stem };
    format!("{stem}.{extension}")
}

/// The `n`th name for a stem when the first is taken: "Title (2).flac".
pub fn numbered(stem: &str, extension: &str, n: u32) -> String {
    if n <= 1 {
        return file_name(stem, extension);
    }
    let suffix = format!(" ({n})");
    let safe = safe_part(stem);
    let extension_room = if extension.trim().is_empty() {
        0
    } else {
        extension.trim().trim_start_matches('.').len().min(8) + 1
    };
    let room = MAX_PART.saturating_sub(suffix.len() + extension_room);
    file_name(&format!("{}{suffix}", cut(&safe, room)), extension)
}

/// The first of "Title.flac", "Title (2).flac", "Title (3).flac", … that
/// `taken` does not claim (the caller asks the file system, which knows
/// whether names differing only in case clash).
pub fn unique_file_name(stem: &str, extension: &str, taken: impl Fn(&str) -> bool) -> String {
    for n in 1..10_000 {
        let name = numbered(stem, extension, n);
        if !taken(&name) {
            return name;
        }
    }
    numbered(stem, extension, 10_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song() -> NameFields {
        NameFields {
            artist: Some("Invented Artist feat. Someone".into()),
            album_artist: Some("Invented Artist".into()),
            album: Some("Invented Album".into()),
            title: "Invented Song".into(),
            track: Some(7),
            disc: Some(1),
            year: Some(2021),
            source: Some("Jellyfin".into()),
        }
    }

    /// The golden table: every platform's rules, one row per rule.
    #[test]
    fn safe_names_follow_every_platforms_rules() {
        let table: &[(&str, &str, &str)] = &[
            ("plain", "Blue Hour", "Blue Hour"),
            ("unicode kept", "東京 Ünïcødé ♫", "東京 Ünïcødé ♫"),
            ("windows: < >", "a<b>c", "a_b_c"),
            ("windows: colon", "Live: 1999", "Live_ 1999"),
            ("windows: quote", "Say \"Hi\"", "Say _Hi_"),
            ("all: slash", "AC/DC", "AC_DC"),
            ("windows: backslash", "a\\b", "a_b"),
            ("windows: pipe", "a|b", "a_b"),
            ("windows: question mark", "Why?", "Why_"),
            ("windows: star", "*NSYNC", "_NSYNC"),
            ("all: control characters", "a\u{0}b\tc\u{7f}d\u{9b}e", "a_b_c_d_e"),
            ("windows: trailing dots", "Vol. 2...", "Vol. 2"),
            ("windows: trailing spaces", "Song   ", "Song"),
            ("leading spaces", "   Song", "Song"),
            ("unix: no hidden files", ".hidden", "_hidden"),
            ("unix: dot dot", "..", "_"),
            ("windows: reserved CON", "CON", "CON_"),
            ("windows: reserved, any case", "nul", "nul_"),
            ("windows: reserved with an extension", "Aux.txt", "Aux_.txt"),
            ("windows: reserved COM1", "com1", "com1_"),
            ("windows: reserved LPT9", "LPT9", "LPT9_"),
            ("windows: reserved superscript", "COM¹", "COM¹_"),
            ("windows: reserved console", "CONOUT$", "CONOUT$_"),
            ("not reserved: longer", "CONSOLE", "CONSOLE"),
            ("not reserved: COM10", "COM10", "COM10"),
            ("nothing left", "   ", "_"),
            ("only dots", "...", "_"),
        ];
        for (rule, input, expected) in table {
            assert_eq!(safe_part(input), *expected, "{rule}");
        }
    }

    #[test]
    fn parts_are_at_most_255_bytes_at_a_character_boundary() {
        let long = "é".repeat(200); // 400 bytes
        let safe = safe_part(&long);
        assert!(safe.len() <= MAX_PART && safe.len() >= MAX_PART - 1);
        assert!(safe.chars().all(|c| c == 'é'));
        let dotted = format!("{}. x", "a".repeat(253)); // the cut lands after ". "
        assert_eq!(safe_part(&dotted), "a".repeat(253));
        let name = file_name(&"b".repeat(400), "flac");
        assert_eq!(name.len(), MAX_PART);
        assert!(name.ends_with(".flac"));
    }

    #[test]
    fn templates_fill_in_the_fields() {
        let table: &[(&str, &[&str])] = &[
            (
                DEFAULT_TEMPLATE,
                &["Invented Artist", "Invented Album", "07 Invented Song"],
            ),
            (
                "{artist} - {title}",
                &["Invented Artist feat. Someone - Invented Song"],
            ),
            (
                "{source}/{year}/{disc}-{track} {title}",
                &["Jellyfin", "2021", "1-07 Invented Song"],
            ),
            ("", &["Invented Artist", "Invented Album", "07 Invented Song"]),
            (
                "{album}/{nope} {title}",
                &["Invented Album", "{nope} Invented Song"],
            ),
            ("a\\b/{title}", &["a", "b", "Invented Song"]),
        ];
        for (template, expected) in table {
            assert_eq!(render(template, &song()), *expected, "{template:?}");
        }
    }

    #[test]
    fn empty_fields_leave_no_gaps_or_empty_folders() {
        let bare = NameFields {
            title: "Song: Live?".into(),
            ..NameFields::default()
        };
        assert_eq!(
            render(DEFAULT_TEMPLATE, &bare),
            ["Unknown Artist", "Unknown Album", "Song_ Live_"]
        );
        assert_eq!(render("{year}/{track} - {title}", &bare), ["Song_ Live_"]);
        let artist_only = NameFields {
            artist: Some("Someone".into()),
            ..bare.clone()
        };
        assert_eq!(
            render("{album_artist}", &artist_only),
            ["Someone"],
            "falls back to the artist"
        );
        assert_eq!(render("{year}", &bare), ["Song_ Live_"], "never no name at all");
        let slashed = NameFields {
            album: Some("Live/Unplugged".into()),
            ..bare
        };
        assert_eq!(
            render("{album}", &slashed),
            ["Live_Unplugged"],
            "a value makes no folder"
        );
    }

    #[test]
    fn taken_names_are_numbered_before_the_extension() {
        let taken = ["Song.flac", "Song (2).flac"];
        assert_eq!(
            unique_file_name("Song", "flac", |n| taken.contains(&n)),
            "Song (3).flac"
        );
        assert_eq!(unique_file_name("Song", ".FLAC", |_| false), "Song.flac");
        let long = "c".repeat(300);
        let second = numbered(&long, "mp3", 2);
        assert_eq!(second.len(), MAX_PART);
        assert!(second.ends_with(" (2).mp3"), "{second}");
        assert_eq!(numbered("CON", "", 2), "CON_ (2)");
    }
}
