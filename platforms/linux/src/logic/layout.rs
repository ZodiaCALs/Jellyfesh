//! Where the player goes (design-controls.md, "Mini-player and the desktop
//! player bar", "Library"): the floating mini-player capsule under 900 px,
//! the player bar from 900 px, the room pages leave for it, the capsule's
//! progress line, and how far a large page title has scrolled away.

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

    /// The room a scrolled page leaves under its last row so the floating
    /// player never covers it: the player and its margins (`space.s4`
    /// below, the same above).
    pub fn bottom_room(self) -> i32 {
        (self.height() + 2.0 * f64::from(space::S4)).round() as i32
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

/// How much of a large title (its height `title_height`, scrolled by
/// `scrolled` px) has gone under the header bar, 0 to 1: the header bar's
/// own small title fades in as the large one leaves.
pub fn title_hidden(scrolled: f64, title_top: f64, title_height: f64) -> f64 {
    if title_height <= 0.0 {
        return 1.0;
    }
    ((scrolled - title_top) / title_height).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_starts_at_900_px() {
        assert_eq!(PLAYER_BAR_MIN_WIDTH, 900);
        assert_eq!(bar_condition(), "min-width: 900px");
        assert_eq!(compact_bar_condition(), "min-width: 900px and max-width: 1099px");
    }

    #[test]
    fn players_follow_the_tokens() {
        assert_eq!(Chrome::Mini.height(), 64.0);
        assert_eq!(Chrome::Bar.height(), 80.0);
        assert_eq!(Chrome::Mini.bottom_room(), 96);
        assert_eq!(Chrome::Bar.bottom_room(), 112);
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
    fn the_small_title_fades_in_as_the_large_one_leaves() {
        assert_eq!(title_hidden(0.0, 16.0, 40.0), 0.0);
        assert_eq!(title_hidden(36.0, 16.0, 40.0), 0.5);
        assert_eq!(title_hidden(400.0, 16.0, 40.0), 1.0);
        assert_eq!(title_hidden(0.0, 16.0, 0.0), 1.0);
    }
}
