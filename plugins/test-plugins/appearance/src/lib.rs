//! A test plugin for `appearance.look` (API 1.2): the look is the JSON in
//! its `look` setting, so a test can send any value, out-of-range ones
//! included; `nan_glass` sets the glass to NaN, which JSON cannot carry.

use std::cell::RefCell;

use jf_plugin_sdk::prelude::*;

struct Appearance;

thread_local! {
    static LOOK: RefCell<Result<Look>> = RefCell::new(Ok(empty()));
}

/// A look that keeps every default.
fn empty() -> Look {
    Look {
        corner_scale: None,
        glass: None,
        aurora: None,
        typeface: None,
        text_scale: None,
        density: None,
        lyrics_weight: None,
        lyrics_glow: None,
        lyrics_highlight: None,
        sidebar: None,
        home: None,
        now_playing: None,
        mini_player: None,
    }
}

impl Plugin for Appearance {
    fn capabilities() -> Vec<Capability> {
        Vec::new()
    }

    fn settings_changed(settings: &serde_json::Value) -> Result<()> {
        let text = settings["look"].as_str().unwrap_or("{}");
        let look = serde_json::from_str::<Look>(text)
            .map(|mut look| {
                if settings["nan_glass"].as_bool() == Some(true) {
                    look.glass = Some(f32::NAN);
                }
                look
            })
            .map_err(|e| PluginError::invalid_input(format!("look: {e}")));
        LOOK.with(|l| *l.borrow_mut() = look);
        Ok(())
    }

    fn look() -> Result<Look> {
        LOOK.with(|l| l.borrow().clone())
    }
}

jf_plugin_sdk::export_plugin!(Appearance);
