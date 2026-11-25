//! Navigation (docs/ui.md, "Navigation"; ADR-0023): Jellyfesh Web's
//! layout. From 761 px wide, the floating sidebar panel lists the library's
//! places, a separator, Search and Sources, with Open Files… and Settings at
//! its foot; up to 760 px, the phone tabs Home · Library · Search · Sources ·
//! Settings, where Library switches between the library's places.
//!
//! The order of the places is the look's (ADR-0020), from the core; this
//! module only says which section each tab and row stands for.

use jf_core::Place;

/// The widest window (px) with the phone layout; from one pixel more the
/// sidebar panel shows (the web's `max-width: 760px`).
pub const PHONE_MAX_WIDTH: u32 = 760;

/// The breakpoint condition for the phone layout (libadwaita syntax).
pub fn phone_condition() -> String {
    format!("max-width: {PHONE_MAX_WIDTH}px")
}

/// A place the window shows: the sidebar's rows, the foot's Settings, and
/// the pages the phone tabs open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    Home,
    Albums,
    Artists,
    Songs,
    Playlists,
    Favourites,
    Downloads,
    Search,
    Sources,
    Settings,
}

impl Section {
    pub const ALL: [Section; 10] = [
        Section::Home,
        Section::Albums,
        Section::Artists,
        Section::Songs,
        Section::Playlists,
        Section::Favourites,
        Section::Downloads,
        Section::Search,
        Section::Sources,
        Section::Settings,
    ];

    /// Its name in scripted entry points and widget names.
    pub fn tag(self) -> &'static str {
        match self {
            Section::Home => "home",
            Section::Albums => "albums",
            Section::Artists => "artists",
            Section::Songs => "songs",
            Section::Playlists => "playlists",
            Section::Favourites => "favourites",
            Section::Downloads => "downloads",
            Section::Search => "search",
            Section::Sources => "sources",
            Section::Settings => "settings",
        }
    }

    pub fn from_tag(tag: &str) -> Option<Section> {
        Section::ALL.into_iter().find(|s| s.tag() == tag)
    }

    /// Its glyph (`ui::widgets::glyph`), as the web's icons.
    pub fn glyph(self) -> &'static str {
        match self {
            Section::Home => "home",
            Section::Albums => "album",
            Section::Artists => "artist",
            Section::Songs => "song",
            Section::Playlists => "playlist",
            Section::Favourites => "heart",
            Section::Downloads => "download",
            Section::Search => "search",
            Section::Sources => "sources",
            Section::Settings => "settings",
        }
    }

    /// The library's places, before the separator.
    pub fn is_library(self) -> bool {
        !matches!(self, Section::Search | Section::Sources | Section::Settings)
    }

    /// The places the phone layout's Library tab switches between.
    pub fn in_library_tab(self) -> bool {
        LIBRARY_SWITCHER.contains(&self)
    }

    /// The section for one of the look's places.
    pub fn of(place: Place) -> Section {
        match place {
            Place::Home => Section::Home,
            Place::Albums => Section::Albums,
            Place::Artists => Section::Artists,
            Place::Songs => Section::Songs,
            Place::Playlists => Section::Playlists,
            Place::Favourites => Section::Favourites,
            Place::Search => Section::Search,
            Place::Sources => Section::Sources,
        }
    }
}

/// What the Library tab switches between, in order (docs/ui.md).
pub const LIBRARY_SWITCHER: [Section; 6] = [
    Section::Albums,
    Section::Artists,
    Section::Songs,
    Section::Playlists,
    Section::Favourites,
    Section::Downloads,
];

/// The sidebar's rows above its foot, in order: the look's places (each
/// once), with Downloads after the last of the library's (the look has no
/// place for it), and a separator before Search or Sources where one
/// follows a library place (`true` at that index).
pub fn sidebar(places: &[Place]) -> Vec<(Section, bool)> {
    let mut sections: Vec<Section> = Vec::new();
    for p in places {
        let s = Section::of(*p);
        if !sections.contains(&s) {
            sections.push(s);
        }
    }
    let last_library = sections.iter().rposition(|s| s.is_library());
    match last_library {
        Some(i) => sections.insert(i + 1, Section::Downloads),
        None => sections.insert(0, Section::Downloads),
    }
    let mut rows: Vec<(Section, bool)> = Vec::with_capacity(sections.len());
    for (i, s) in sections.iter().enumerate() {
        let separated = !s.is_library() && i > 0 && sections[i - 1].is_library();
        rows.push((*s, separated));
    }
    rows
}

/// A phone tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tab {
    Home,
    Library,
    Search,
    Sources,
    Settings,
}

impl Tab {
    pub fn tag(self) -> &'static str {
        match self {
            Tab::Home => "home",
            Tab::Library => "library",
            Tab::Search => "search",
            Tab::Sources => "sources",
            Tab::Settings => "settings",
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            Tab::Home => "home",
            Tab::Library => "library",
            Tab::Search => "search",
            Tab::Sources => "sources",
            Tab::Settings => "settings",
        }
    }
}

/// The tab a section shows under: the library's places are under Library.
pub fn tab_of(section: Section) -> Tab {
    match section {
        Section::Home => Tab::Home,
        Section::Search => Tab::Search,
        Section::Sources => Tab::Sources,
        Section::Settings => Tab::Settings,
        _ => Tab::Library,
    }
}

/// The phone tabs, in order: the look's places as tabs (Library where the
/// first of the library's places is), each once, then Settings.
pub fn tabs(places: &[Place]) -> Vec<Tab> {
    let mut out: Vec<Tab> = Vec::new();
    for p in places {
        let tab = tab_of(Section::of(*p));
        if !out.contains(&tab) {
            out.push(tab);
        }
    }
    if !out.contains(&Tab::Library) {
        // Downloads is always in the library.
        out.push(Tab::Library);
    }
    out.push(Tab::Settings);
    out
}

/// Settings' sections, in every app's order (docs/ui.md, "Settings").
pub const SETTINGS_SECTIONS: [&str; 6] = [
    "appearance",
    "playback",
    "library",
    "sources",
    "extensions",
    "about",
];

/// The Settings section a name opens: Lyrics is in Appearance, the keyboard
/// shortcuts are in About; `None` for a name that is no section.
pub fn settings_section(name: &str) -> Option<&'static str> {
    match name {
        "lyrics" => Some("appearance"),
        "shortcuts" | "keyboard" => Some("about"),
        other => SETTINGS_SECTIONS.iter().copied().find(|s| *s == other),
    }
}

/// What the Library tab opens: the library place shown last, or Albums.
pub fn library_tab_target(last: Option<Section>) -> Section {
    match last {
        Some(s) if s.in_library_tab() => s,
        _ => Section::Albums,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LUMINOUS: [Place; 8] = [
        Place::Home,
        Place::Albums,
        Place::Artists,
        Place::Songs,
        Place::Playlists,
        Place::Favourites,
        Place::Search,
        Place::Sources,
    ];

    #[test]
    fn the_phone_layout_ends_at_760_px() {
        assert_eq!(PHONE_MAX_WIDTH, 760);
        assert_eq!(phone_condition(), "max-width: 760px");
    }

    #[test]
    fn the_sidebar_lists_the_webs_places_with_downloads() {
        let rows = sidebar(&LUMINOUS);
        let sections: Vec<Section> = rows.iter().map(|(s, _)| *s).collect();
        assert_eq!(
            sections,
            vec![
                Section::Home,
                Section::Albums,
                Section::Artists,
                Section::Songs,
                Section::Playlists,
                Section::Favourites,
                Section::Downloads,
                Section::Search,
                Section::Sources,
            ]
        );
        // One separator, before Search.
        let separated: Vec<Section> = rows.iter().filter(|(_, sep)| *sep).map(|(s, _)| *s).collect();
        assert_eq!(separated, vec![Section::Search]);
    }

    #[test]
    fn a_customized_order_keeps_downloads_after_the_library() {
        let rows = sidebar(&[Place::Songs, Place::Search, Place::Albums, Place::Songs]);
        let sections: Vec<Section> = rows.iter().map(|(s, _)| *s).collect();
        assert_eq!(
            sections,
            vec![
                Section::Songs,
                Section::Search,
                Section::Albums,
                Section::Downloads
            ]
        );
        let separated: Vec<bool> = rows.iter().map(|(_, sep)| *sep).collect();
        assert_eq!(separated, vec![false, true, false, false]);
        // No library place listed: Downloads first.
        let rows = sidebar(&[Place::Search]);
        assert_eq!(rows, vec![(Section::Downloads, false), (Section::Search, true)]);
    }

    #[test]
    fn phone_tabs_put_the_library_in_one_tab_and_settings_last() {
        assert_eq!(
            tabs(&LUMINOUS),
            vec![Tab::Home, Tab::Library, Tab::Search, Tab::Sources, Tab::Settings]
        );
        assert_eq!(
            tabs(&[Place::Search, Place::Songs, Place::Home]),
            vec![Tab::Search, Tab::Library, Tab::Home, Tab::Settings]
        );
        assert_eq!(
            tabs(&[Place::Sources]),
            vec![Tab::Sources, Tab::Library, Tab::Settings]
        );
    }

    #[test]
    fn sections_show_under_their_tab() {
        for s in LIBRARY_SWITCHER {
            assert_eq!(tab_of(s), Tab::Library, "{s:?}");
            assert!(s.in_library_tab());
            assert!(s.is_library());
        }
        assert_eq!(tab_of(Section::Home), Tab::Home);
        assert!(!Section::Home.in_library_tab());
        assert_eq!(tab_of(Section::Settings), Tab::Settings);
        assert!(!Section::Settings.is_library());
        assert!(!Section::Search.is_library());
    }

    #[test]
    fn the_library_tab_returns_to_the_last_library_place() {
        assert_eq!(library_tab_target(None), Section::Albums);
        assert_eq!(library_tab_target(Some(Section::Songs)), Section::Songs);
        assert_eq!(library_tab_target(Some(Section::Downloads)), Section::Downloads);
        assert_eq!(library_tab_target(Some(Section::Search)), Section::Albums);
        assert_eq!(library_tab_target(Some(Section::Home)), Section::Albums);
    }

    #[test]
    fn settings_sections_in_every_apps_order() {
        assert_eq!(
            SETTINGS_SECTIONS,
            [
                "appearance",
                "playback",
                "library",
                "sources",
                "extensions",
                "about"
            ]
        );
        assert_eq!(settings_section("lyrics"), Some("appearance"));
        assert_eq!(settings_section("shortcuts"), Some("about"));
        assert_eq!(settings_section("sources"), Some("sources"));
        assert_eq!(settings_section("preferences"), None);
    }

    #[test]
    fn tags_name_every_section_once() {
        for s in Section::ALL {
            assert_eq!(Section::from_tag(s.tag()), Some(s));
        }
        assert_eq!(Section::from_tag("preferences"), None);
    }
}
