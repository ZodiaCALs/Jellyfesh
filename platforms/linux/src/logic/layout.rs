//! Where the player goes (design-controls.md, "Mini-player and the desktop
//! player bar", "Library"): the floating mini-player capsule under 900 px,
//! the player bar from 900 px, the room pages leave for it, the capsule's
//! progress line, how far a large page title has scrolled away, and the
//! bands where content fades out (Luminous: the pages fade out just above
//! the floating player; the lyrics fade at their ends).

use jf_core::{MiniPlayerStyle, NowPlayingLayout};
use jf_tokens::{player, space};

use crate::logic::seek::Line;

/// The narrowest window (px) that shows the player bar; narrower windows
/// show the mini-player capsule.
pub const PLAYER_BAR_MIN_WIDTH: u32 = 900;
/// The narrowest window (px) whose player bar has room for the volume
/// line; between the two, the bar keeps only the mute button.
pub const PLAYER_BAR_FULL_WIDTH: u32 = 1100;
const _: () = assert!(PLAYER_BAR_FULL_WIDTH > PLAYER_BAR_MIN_WIDTH);

/// The floating player along the bottom of the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chrome {
    /// The capsule: art, two lines of type, one play button (CMP-02).
    Mini,
    /// The desktop player bar: art and titles, transport and seek bar,
    /// Lyrics, Queue and volume.
    Bar,
}

impl Chrome {
    /// The player's height (player tokens).
    pub fn height(self) -> f64 {
        f64::from(match self {
            Chrome::Mini => player::MINI_HEIGHT,
            Chrome::Bar => player::BAR_HEIGHT,
        })
    }
}

/// The phone tab bar (Jellyfesh Web's `.tabs`): 68 px high, `space.s6`
/// above the window's bottom edge, 14 px from its sides.
pub const TABS_HEIGHT: i32 = 68;
pub const TABS_BOTTOM: i32 = space::S6 as i32;
pub const TABS_SIDE: i32 = 14;

/// How far above the window's bottom edge the player floats: `space.s4`,
/// or over the phone tabs with `space.s3` between (the web's dock).
pub fn dock_bottom(phone: bool) -> i32 {
    if phone {
        TABS_HEIGHT + TABS_BOTTOM + space::S3 as i32
    } else {
        space::S4 as i32
    }
}

/// The room a scrolled page leaves under its last row, so the floating
/// player (`chrome`, while something is queued) and the phone tabs never
/// cover it: everything up to the top of the highest of them, and
/// `space.s4` more.
pub fn bottom_room(chrome: Option<Chrome>, phone: bool) -> i32 {
    let gap = space::S4 as i32;
    match chrome {
        Some(c) => dock_bottom(phone) + c.height().round() as i32 + gap,
        None if phone => TABS_HEIGHT + TABS_BOTTOM + gap,
        None => 0,
    }
}

/// The content's side gutters (the web's `layout.gutter-fluid`, at most
/// 32 px from 800 px wide, and `space.s4` on phones). Between 761 and 800 px
/// the web's is a pixel or two less; the app keeps 32.
pub fn gutter(phone: bool) -> i32 {
    if phone {
        space::S4 as i32
    } else {
        jf_tokens::layout::GUTTER_FLUID.max as i32
    }
}

/// The breakpoint condition for the player bar (libadwaita syntax).
pub fn bar_condition() -> String {
    format!("min-width: {PLAYER_BAR_MIN_WIDTH}px")
}

/// The breakpoint condition for the narrower player bar, without the volume
/// line. Added after [`bar_condition`]'s, it wins where both hold.
pub fn compact_bar_condition() -> String {
    format!(
        "min-width: {PLAYER_BAR_MIN_WIDTH}px and max-width: {}px",
        PLAYER_BAR_FULL_WIDTH - 1
    )
}

/// The mini-player's progress line (`player.mini-progress`, 2 px) along
/// the capsule's bottom edge: inset from both ends by the capsule's corner
/// radius, and never less than `player.mini-radius`, so it runs along the
/// straight part of the edge; flush with the edge.
pub fn mini_line(width: f64, height: f64, corner_radius: f64) -> Line {
    let radius = corner_radius.min(height / 2.0).max(0.0);
    let inset = radius.max(f64::from(player::MINI_RADIUS));
    let thickness = f64::from(player::MINI_PROGRESS);
    let x0 = inset.min(width / 2.0);
    Line {
        x0,
        x1: (width - inset).max(x0),
        y: height - thickness / 2.0,
    }
}

/// Album covers in the grid (Luminous): about this wide, four per row in
/// the default window (the approved mock-up's 214 px).
pub const GRID_COVER: f64 = 214.0;
/// The narrowest a grid cover gets, so two columns fit a 360 px window.
pub const GRID_COVER_MIN: i32 = 140;
/// Room around each cover in the grid (`space.s2` each side).
const GRID_CELL_PADDING: f64 = 2.0 * space::S2 as f64;

/// How many columns of covers a grid `width` px wide shows: as many as
/// keep them near `cover` px, at least two and at most twelve.
pub fn grid_columns(width: f64, cover: f64) -> u32 {
    if width <= 0.0 || cover <= 0.0 {
        return 2;
    }
    (width / (cover + GRID_CELL_PADDING)).round().clamp(2.0, 12.0) as u32
}

/// How far above the floating player the pages fade out (Luminous): the
/// player's margin (`space.s4`) and as much again, so the last visible row
/// dissolves just before the capsule.
pub const PLAYER_FADE: f64 = 2.0 * space::S4 as f64;

/// The fade bands (top, bottom) for content `height` px high, each at most
/// what was asked and together never more than the height, so short views
/// keep their middle opaque. Negative requests count as none.
pub fn fade_bands(height: f64, top: f64, bottom: f64) -> (f64, f64) {
    let (top, bottom) = (top.max(0.0), bottom.max(0.0));
    let height = height.max(0.0);
    let total = top + bottom;
    if total <= height || total == 0.0 {
        (top, bottom)
    } else {
        let k = height / total;
        (top * k, bottom * k)
    }
}

/// How much of a large title (its height `title_height`, scrolled by
/// `scrolled` px) has gone under the header bar, 0 to 1: the header bar's
/// own small title fades in as the large one leaves.
pub fn title_hidden(scrolled: f64, title_top: f64, title_height: f64) -> f64 {
    if title_height <= 0.0 {
        return 1.0;
    }
    ((scrolled - title_top) / title_height).clamp(0.0, 1.0)
}

/// The player at the bottom for the look's style (ADR-0020), and whether
/// it floats at the trailing corner: Bar is v0.0.6 (the bar in wide
/// windows, the capsule in narrow ones), Compact the capsule at every
/// width, Floating a smaller capsule at the trailing corner.
pub fn chrome_for(style: MiniPlayerStyle, wide: bool) -> (Chrome, bool) {
    match style {
        MiniPlayerStyle::Bar => (if wide { Chrome::Bar } else { Chrome::Mini }, false),
        MiniPlayerStyle::Compact => (Chrome::Mini, false),
        MiniPlayerStyle::Floating => (Chrome::Mini, true),
    }
}

/// The floating capsule's widest (px); the capsule at the bottom centre
/// is up to 560 px.
pub const FLOATING_WIDTH: i32 = 360;

/// How Now Playing arranges itself for the look's layout (ADR-0020).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrangement {
    /// The v0.0.6 stage: the artwork beside the pane.
    Stage,
    /// A large cover with the line being sung below it, in place of the
    /// Lyrics pane.
    Cover,
    /// The pane across the view, a small cover in the corner.
    WidePane,
}

/// Stage is v0.0.6. Artwork shows the cover and the sung line in place of
/// the Lyrics pane, and Queue, Info and Comments beside the cover as on the
/// stage. Lyrics puts any pane across the view. Narrow views keep their
/// one-thing-at-a-time layout.
pub fn arrangement(layout: NowPlayingLayout, lyrics_shown: bool, narrow: bool) -> Arrangement {
    if narrow {
        return Arrangement::Stage;
    }
    match layout {
        NowPlayingLayout::Stage => Arrangement::Stage,
        NowPlayingLayout::Artwork if lyrics_shown => Arrangement::Cover,
        NowPlayingLayout::Artwork => Arrangement::Stage,
        NowPlayingLayout::Lyrics => Arrangement::WidePane,
    }
}

/// The cover in the Artwork layout: up to 520 px, as the view allows (the
/// top bar, the dock, the title on up to two lines and the sung line with
/// its translation take about 480 px of the height, and the cover at most
/// half the width), never under 180.
pub fn cover_side(width: i32, height: i32) -> i32 {
    520.min(width / 2).min(height - 480).max(180)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_player_for_the_looks_style() {
        assert_eq!(chrome_for(MiniPlayerStyle::Bar, true), (Chrome::Bar, false));
        assert_eq!(chrome_for(MiniPlayerStyle::Bar, false), (Chrome::Mini, false));
        assert_eq!(chrome_for(MiniPlayerStyle::Compact, true), (Chrome::Mini, false));
        assert_eq!(chrome_for(MiniPlayerStyle::Floating, true), (Chrome::Mini, true));
        assert_eq!(chrome_for(MiniPlayerStyle::Floating, false), (Chrome::Mini, true));
    }

    #[test]
    fn now_playing_arrangements() {
        use NowPlayingLayout::*;
        assert_eq!(arrangement(Stage, true, false), Arrangement::Stage);
        assert_eq!(arrangement(Artwork, true, false), Arrangement::Cover);
        assert_eq!(arrangement(Artwork, false, false), Arrangement::Stage);
        assert_eq!(arrangement(Lyrics, false, false), Arrangement::WidePane);
        assert_eq!(
            arrangement(Lyrics, true, true),
            Arrangement::Stage,
            "narrow views"
        );
    }

    #[test]
    fn the_cover_fits_the_view() {
        assert_eq!(cover_side(1600, 1100), 520);
        assert_eq!(cover_side(1200, 780), 300);
        assert_eq!(cover_side(700, 1000), 350);
        assert_eq!(cover_side(1200, 500), 180);
    }

    #[test]
    fn the_bar_starts_at_900_px() {
        assert_eq!(PLAYER_BAR_MIN_WIDTH, 900);
        assert_eq!(bar_condition(), "min-width: 900px");
        assert_eq!(compact_bar_condition(), "min-width: 900px and max-width: 1099px");
    }

    #[test]
    fn the_dock_floats_over_the_phone_tabs() {
        assert_eq!(dock_bottom(false), 16);
        assert_eq!(dock_bottom(true), 104);
        // Wide: the player and s4 above and below it, as before.
        assert_eq!(bottom_room(Some(Chrome::Bar), false), 112);
        assert_eq!(bottom_room(Some(Chrome::Mini), false), 96);
        assert_eq!(bottom_room(None, false), 0);
        // Phones: the tabs (68 + 24), the capsule 12 above them, s4 above.
        assert_eq!(bottom_room(Some(Chrome::Mini), true), 184);
        assert_eq!(bottom_room(None, true), 108);
        assert_eq!(gutter(false), 32);
        assert_eq!(gutter(true), 16);
    }

    #[test]
    fn players_follow_the_tokens() {
        assert_eq!(Chrome::Mini.height(), 64.0);
        assert_eq!(Chrome::Bar.height(), 80.0);
    }

    #[test]
    fn the_progress_line_runs_along_the_straight_bottom_edge() {
        // The capsule is a pill: 64 px high, so its corners are 32 px.
        let line = mini_line(560.0, 64.0, 999.0);
        assert_eq!(line.x0, 32.0);
        assert_eq!(line.x1, 528.0);
        assert_eq!(line.y, 63.0);
        // Rounder corners than mini-radius never shorten the inset below it.
        let square = mini_line(560.0, 64.0, 0.0);
        assert_eq!(square.x0, 16.0);
        assert_eq!(square.x1, 544.0);
        // A capsule narrower than its corners has a line of no length.
        let tiny = mini_line(40.0, 64.0, 999.0);
        assert_eq!(tiny.length(), 0.0);
    }

    #[test]
    fn grids_show_about_four_large_covers_in_the_default_window() {
        // The default 1200 px window less the sidebar and the grid's
        // padding leaves about 880 px: four covers.
        assert_eq!(grid_columns(880.0, GRID_COVER), 4);
        // The mock-up's 1280 px window.
        assert_eq!(grid_columns(984.0, GRID_COVER), 4);
        assert_eq!(grid_columns(1_400.0, GRID_COVER), 6);
        // Phones keep two columns, very wide windows at most twelve.
        assert_eq!(grid_columns(312.0, GRID_COVER), 2);
        assert_eq!(grid_columns(10_000.0, GRID_COVER), 12);
        assert_eq!(grid_columns(0.0, GRID_COVER), 2);
        // Two columns of the narrowest covers fit a 360 px window's grid.
        assert!(2.0 * (f64::from(GRID_COVER_MIN) + GRID_CELL_PADDING) <= 360.0 - 48.0);
    }

    #[test]
    fn fades_fit_in_their_view() {
        assert_eq!(PLAYER_FADE, 32.0);
        assert_eq!(fade_bands(600.0, 0.0, PLAYER_FADE), (0.0, 32.0));
        assert_eq!(fade_bands(600.0, 80.0, 120.0), (80.0, 120.0));
        // A view shorter than both bands shares its height between them.
        assert_eq!(fade_bands(100.0, 80.0, 120.0), (40.0, 60.0));
        assert_eq!(fade_bands(0.0, 10.0, 10.0), (0.0, 0.0));
        assert_eq!(fade_bands(100.0, -5.0, 0.0), (0.0, 0.0));
    }

    #[test]
    fn the_small_title_fades_in_as_the_large_one_leaves() {
        assert_eq!(title_hidden(0.0, 16.0, 40.0), 0.0);
        assert_eq!(title_hidden(36.0, 16.0, 40.0), 0.5);
        assert_eq!(title_hidden(400.0, 16.0, 40.0), 1.0);
        assert_eq!(title_hidden(0.0, 16.0, 0.0), 1.0);
    }
}
