//! Drag-and-drop reordering for the queue and playlists.
//!
//! `Command::MoveInQueue { to }` (and `MovePlaylistEntry`) take the index the
//! entry should have once it has moved. A drop lands before or after a row;
//! these helpers turn that into the `to` index.

/// Where a dragged row was dropped relative to the row under the pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drop {
    Before,
    After,
}

impl Drop {
    /// Before when the pointer is in the upper half of a row of `height`.
    pub fn at(y: f64, height: f64) -> Drop {
        if y < height / 2.0 {
            Drop::Before
        } else {
            Drop::After
        }
    }
}

/// The index for the move command when the row at `from` is dropped
/// `side` of the row at `target`, in a list of `len` rows. `None` when the
/// drop leaves the order unchanged.
pub fn move_target(from: usize, target: usize, side: Drop, len: usize) -> Option<usize> {
    if len == 0 || from >= len || target >= len {
        return None;
    }
    let to = match side {
        Drop::Before if from < target => target - 1,
        Drop::Before => target,
        Drop::After if from <= target => target,
        Drop::After => target + 1,
    };
    let to = to.min(len - 1);
    (to != from).then_some(to)
}

/// The playback index of row `row` in an "up next" list that starts right
/// after the current entry.
pub fn up_next_index(position: Option<usize>, row: usize) -> usize {
    position.map_or(0, |p| p + 1) + row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_above_and_below() {
        // Rows a b c d e; drag b (1).
        assert_eq!(move_target(1, 3, Drop::Before, 5), Some(2)); // a c b d e
        assert_eq!(move_target(1, 3, Drop::After, 5), Some(3)); // a c d b e
        assert_eq!(move_target(1, 0, Drop::Before, 5), Some(0)); // b a c d e
        assert_eq!(move_target(1, 4, Drop::After, 5), Some(4)); // a c d e b
        // Drag d (3) up.
        assert_eq!(move_target(3, 1, Drop::Before, 5), Some(1));
        assert_eq!(move_target(3, 1, Drop::After, 5), Some(2));
    }

    #[test]
    fn no_op_drops() {
        assert_eq!(move_target(2, 2, Drop::Before, 5), None);
        assert_eq!(move_target(2, 2, Drop::After, 5), None);
        assert_eq!(move_target(2, 3, Drop::Before, 5), None);
        assert_eq!(move_target(2, 1, Drop::After, 5), None);
        assert_eq!(move_target(0, 0, Drop::After, 0), None);
        assert_eq!(move_target(7, 1, Drop::After, 5), None);
    }

    #[test]
    fn halves_and_offsets() {
        assert_eq!(Drop::at(10.0, 52.0), Drop::Before);
        assert_eq!(Drop::at(30.0, 52.0), Drop::After);
        assert_eq!(up_next_index(Some(4), 0), 5);
        assert_eq!(up_next_index(None, 2), 2);
    }
}
