//! Hostile test plugin: memory bombs.
//!
//! * `search`: one 256 MiB allocation, touched;
//! * `browse`: grows a list 1 MiB at a time until something gives;
//! * `recommendations_for_user`: answers at once (proves restarts work).

use std::hint::black_box;

use jf_plugin_sdk::prelude::*;

struct Bomb;

impl Plugin for Bomb {
    fn capabilities() -> Vec<Capability> {
        vec![
            Capability::Search,
            Capability::Browse,
            Capability::Recommendations,
        ]
    }

    fn search(_query: String, _filters: SearchFilters, _page: PageRequest) -> Result<ItemPage> {
        let mut big = vec![0u8; 256 * 1024 * 1024];
        for i in (0..big.len()).step_by(4096) {
            big[i] = 1;
        }
        black_box(&big);
        Ok(ItemPage::new(Vec::new()))
    }

    fn browse(_node: Option<Item>, _page: PageRequest) -> Result<ItemPage> {
        let mut chunks: Vec<Vec<u8>> = Vec::new();
        loop {
            chunks.push(vec![7u8; 1024 * 1024]);
            black_box(&chunks);
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

jf_plugin_sdk::export_plugin!(Bomb);
