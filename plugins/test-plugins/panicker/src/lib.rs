//! Hostile test plugin: crashes.
//!
//! * `search`: `panic!` with the query in the message;
//! * `browse`: aborts without a panic message (`unreachable` trap);
//! * `lyrics`: unbounded recursion (stack overflow);
//! * `recommendations_for_user`: answers at once, counting calls since the
//!   instance started (proves a crash restarts the instance).

use std::cell::Cell;
use std::hint::black_box;

use jf_plugin_sdk::prelude::*;

struct Panicker;

thread_local! {
    static CALLS: Cell<u32> = const { Cell::new(0) };
}

#[allow(unconditional_recursion, reason = "the point of this test plugin")]
fn recurse(depth: u64) -> u64 {
    black_box(recurse(black_box(depth + 1)) + 1)
}

impl Plugin for Panicker {
    fn capabilities() -> Vec<Capability> {
        vec![
            Capability::Search,
            Capability::Browse,
            Capability::Lyrics,
            Capability::Recommendations,
        ]
    }

    fn search(query: String, _filters: SearchFilters, _page: PageRequest) -> Result<ItemPage> {
        panic!("boom: {query}");
    }

    fn browse(_node: Option<Item>, _page: PageRequest) -> Result<ItemPage> {
        std::process::abort();
    }

    fn lyrics(_item: Item) -> Result<Option<LyricPayload>> {
        let depth = recurse(0);
        Ok(Some(LyricPayload::new(LyricFormat::Plain, depth.to_string())))
    }

    fn recommendations_for_user(_page: PageRequest) -> Result<ItemPage> {
        let calls = CALLS.with(|c| {
            c.set(c.get() + 1);
            c.get()
        });
        Ok(ItemPage::new(vec![Item::new(
            ItemKind::Track,
            "calls",
            calls.to_string(),
        )]))
    }
}

jf_plugin_sdk::export_plugin!(Panicker);
