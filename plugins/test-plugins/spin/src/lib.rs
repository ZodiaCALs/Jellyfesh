//! Hostile test plugin: never finishes.
//!
//! * `search`: a pure infinite loop (CPU budget);
//! * `browse`: an infinite loop that keeps calling a cheap host import, so
//!   time spent in the host cannot hide the guest's own CPU use;
//! * `lyrics`: endless HTTP requests to a slow endpoint, which use no CPU
//!   (wall-clock deadline);
//! * `recommendations_for_user`: answers at once (proves restarts work).

use std::cell::RefCell;
use std::hint::black_box;

use jf_plugin_sdk::prelude::*;

struct Spin;

thread_local! {
    static SERVER: RefCell<String> = const { RefCell::new(String::new()) };
}

impl Plugin for Spin {
    fn capabilities() -> Vec<Capability> {
        vec![
            Capability::Search,
            Capability::Browse,
            Capability::Lyrics,
            Capability::Recommendations,
        ]
    }

    fn settings_changed(settings: &serde_json::Value) -> Result<()> {
        let url = settings["server_url"].as_str().unwrap_or_default().to_string();
        SERVER.with(|s| *s.borrow_mut() = url);
        Ok(())
    }

    fn search(query: String, _filters: SearchFilters, _page: PageRequest) -> Result<ItemPage> {
        let mut counter = black_box(query.len() as u64);
        loop {
            counter = black_box(counter.wrapping_mul(6364136223846793005).wrapping_add(1));
        }
    }

    fn browse(_node: Option<Item>, _page: PageRequest) -> Result<ItemPage> {
        let mut total = 0u64;
        loop {
            total = black_box(total.wrapping_add(clock::monotonic_ms()));
        }
    }

    fn lyrics(_item: Item) -> Result<Option<LyricPayload>> {
        let url = format!("{}/slow", SERVER.with(|s| s.borrow().clone()));
        loop {
            let _ = black_box(http::Request::get(url.as_str()).send());
        }
    }

    fn recommendations_for_user(_page: PageRequest) -> Result<ItemPage> {
        Ok(ItemPage::new(vec![Item::new(
            ItemKind::Track,
            "alive",
            "Still alive",
        )]))
    }
}

jf_plugin_sdk::export_plugin!(Spin);
