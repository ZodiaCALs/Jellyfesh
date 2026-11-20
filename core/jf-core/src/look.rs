//! The `Look` slice (ADR-0020 §2): the active appearance plugin's look,
//! checked by the plugin host, with the Luminous defaults filled in for
//! everything it leaves out.

use jf_plugin_host as host;

use crate::types::{
    Density, HomeSection, LookSnapshot, LyricsHighlight, LyricsWeight, MiniPlayerStyle, NowPlayingLayout,
    Place, Typeface,
};

/// The slice for `look` from `plugin` (already checked by the host), or the
/// defaults when there is none.
pub(crate) fn resolve(look: Option<&host::Look>, plugin: Option<&str>, version: u64) -> LookSnapshot {
    let defaults = LookSnapshot::defaults(version);
    let (Some(look), Some(plugin)) = (look, plugin) else {
        return defaults;
    };
    LookSnapshot {
        version,
        corner_scale: look.corner_scale.unwrap_or(defaults.corner_scale),
        glass: look.glass.unwrap_or(defaults.glass),
        aurora: look.aurora.unwrap_or(defaults.aurora),
        typeface: look.typeface.map_or(defaults.typeface, typeface),
        text_scale: look.text_scale.unwrap_or(defaults.text_scale),
        density: look.density.map_or(defaults.density, density),
        lyrics_weight: look.lyrics_weight.map_or(defaults.lyrics_weight, weight),
        lyrics_glow: look.lyrics_glow.unwrap_or(defaults.lyrics_glow),
        lyrics_highlight: look.lyrics_highlight.map_or(defaults.lyrics_highlight, highlight),
        sidebar: look.sidebar.as_ref().map_or(defaults.sidebar, |places| {
            places.iter().map(|p| place(*p)).collect()
        }),
        home: look.home.as_ref().map_or(defaults.home, |sections| {
            sections.iter().map(|s| section(*s)).collect()
        }),
        now_playing: look.now_playing.map_or(defaults.now_playing, layout),
        mini_player: look.mini_player.map_or(defaults.mini_player, mini_player),
        customized: true,
        plugin: Some(plugin.to_owned()),
    }
}

fn typeface(t: host::Typeface) -> Typeface {
    match t {
        host::Typeface::System => Typeface::System,
        host::Typeface::Rounded => Typeface::Rounded,
        host::Typeface::Serif => Typeface::Serif,
        host::Typeface::Mono => Typeface::Mono,
    }
}

fn density(d: host::Density) -> Density {
    match d {
        host::Density::Compact => Density::Compact,
        host::Density::Standard => Density::Standard,
        host::Density::Roomy => Density::Roomy,
    }
}

fn weight(w: host::LyricsWeight) -> LyricsWeight {
    match w {
        host::LyricsWeight::Regular => LyricsWeight::Regular,
        host::LyricsWeight::Medium => LyricsWeight::Medium,
        host::LyricsWeight::Semibold => LyricsWeight::Semibold,
        host::LyricsWeight::Bold => LyricsWeight::Bold,
        host::LyricsWeight::Heavy => LyricsWeight::Heavy,
    }
}

fn highlight(h: host::Highlight) -> LyricsHighlight {
    match h {
        host::Highlight::Artwork => LyricsHighlight::Artwork,
        host::Highlight::Accent => LyricsHighlight::Accent,
        host::Highlight::Custom(rgb) => LyricsHighlight::Custom { rgb },
    }
}

fn place(p: host::Place) -> Place {
    match p {
        host::Place::Home => Place::Home,
        host::Place::Albums => Place::Albums,
        host::Place::Artists => Place::Artists,
        host::Place::Songs => Place::Songs,
        host::Place::Playlists => Place::Playlists,
        host::Place::Favourites => Place::Favourites,
        host::Place::Search => Place::Search,
        host::Place::Sources => Place::Sources,
    }
}

fn section(s: host::HomeSection) -> HomeSection {
    match s {
        host::HomeSection::Summary => HomeSection::Summary,
        host::HomeSection::RecentlyAdded => HomeSection::RecentlyAdded,
        host::HomeSection::RecentlyPlayed => HomeSection::RecentlyPlayed,
        host::HomeSection::MostPlayed => HomeSection::MostPlayed,
        host::HomeSection::Favourites => HomeSection::Favourites,
    }
}

fn layout(l: host::NowPlayingLayout) -> NowPlayingLayout {
    match l {
        host::NowPlayingLayout::Stage => NowPlayingLayout::Stage,
        host::NowPlayingLayout::Artwork => NowPlayingLayout::Artwork,
        host::NowPlayingLayout::Lyrics => NowPlayingLayout::Lyrics,
    }
}

fn mini_player(m: host::MiniPlayerStyle) -> MiniPlayerStyle {
    match m {
        host::MiniPlayerStyle::Bar => MiniPlayerStyle::Bar,
        host::MiniPlayerStyle::Compact => MiniPlayerStyle::Compact,
        host::MiniPlayerStyle::Floating => MiniPlayerStyle::Floating,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_fill_what_the_plugin_leaves_out() {
        let look = host::Look {
            corner_scale: Some(1.5),
            typeface: Some(host::Typeface::Serif),
            sidebar: Some(vec![host::Place::Songs, host::Place::Home]),
            lyrics_highlight: Some(host::Highlight::Custom(0x00ff_8800)),
            ..host::Look::default()
        };
        let resolved = resolve(Some(&look), Some("org.x"), 3);
        assert!(resolved.customized);
        assert_eq!(resolved.corner_scale, 1.5);
        assert_eq!(resolved.glass, 1.0);
        assert_eq!(resolved.aurora, jf_tokens::field::AURORA_STRENGTH);
        assert_eq!(resolved.typeface, Typeface::Serif);
        assert_eq!(resolved.sidebar, vec![Place::Songs, Place::Home]);
        assert_eq!(resolved.home, LookSnapshot::defaults(0).home);
        assert_eq!(
            resolved.lyrics_highlight,
            LyricsHighlight::Custom { rgb: 0x00ff_8800 }
        );
        assert_eq!(resolved.plugin.as_deref(), Some("org.x"));
        let none = resolve(None, None, 4);
        assert_eq!(none, LookSnapshot::defaults(4));
        assert!(!none.customized);
    }
}
