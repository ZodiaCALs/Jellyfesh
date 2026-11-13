//! Hostile test plugin: network attacks. `search(<url>)` requests the URL
//! (with `{server}` replaced by the configured server) and answers with one
//! item whose title is the outcome: `ok <status> <body length>` or
//! `err <http-error>`. The tests choose URLs for oversized responses,
//! redirect escapes, DNS rebinding, private addresses and direct IPs.
//! `browse` sends 50 requests as fast as it can and reports how many
//! were refused by the rate limit.

use std::cell::RefCell;

use jf_plugin_sdk::http::Request;
use jf_plugin_sdk::prelude::*;

struct Attacker;

thread_local! {
    static SERVER: RefCell<String> = const { RefCell::new(String::new()) };
}

fn outcome(url: &str) -> String {
    match Request::get(url).timeout_ms(3000).send() {
        Ok(response) => format!("ok {} {}", response.status, response.body.len()),
        Err(error) => format!("err {}", format!("{error:?}").trim_start_matches("HttpError::")),
    }
}

impl Plugin for Attacker {
    fn capabilities() -> Vec<Capability> {
        vec![Capability::Search, Capability::Browse]
    }

    fn settings_changed(settings: &serde_json::Value) -> Result<()> {
        let url = settings["server_url"]
            .as_str()
            .unwrap_or_default()
            .trim_end_matches('/')
            .to_string();
        SERVER.with(|s| *s.borrow_mut() = url);
        Ok(())
    }

    fn search(query: String, _filters: SearchFilters, _page: PageRequest) -> Result<ItemPage> {
        let server = SERVER.with(|s| s.borrow().clone());
        let url = query.replace("{server}", &server);
        Ok(ItemPage::new(vec![Item::new(
            ItemKind::Track,
            "outcome",
            outcome(&url),
        )]))
    }

    fn browse(_node: Option<Item>, _page: PageRequest) -> Result<ItemPage> {
        let server = SERVER.with(|s| s.borrow().clone());
        let mut refused = 0;
        let mut sent = 0;
        for _ in 0..50 {
            match Request::get(format!("{server}/ping")).send() {
                Ok(_) => sent += 1,
                Err(jf_plugin_sdk::http::HttpError::RateLimited) => refused += 1,
                Err(_) => {}
            }
        }
        Ok(ItemPage::new(vec![Item::new(
            ItemKind::Track,
            "rate",
            format!("sent {sent} refused {refused}"),
        )]))
    }
}

jf_plugin_sdk::export_plugin!(Attacker);
