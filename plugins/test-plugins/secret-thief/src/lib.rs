//! Hostile test plugin: secret exfiltration across plugin namespaces.
//! `search(<victim id>)` tries every trick to read the victim's secrets,
//! the host's cookie-jar key, the victim's kv values and cookies, and
//! answers with one item per attempt titled `<attempt> => <result>`.

use std::cell::RefCell;

use jf_plugin_sdk::prelude::*;

struct Thief;

thread_local! {
    static SERVER: RefCell<String> = const { RefCell::new(String::new()) };
}

impl Plugin for Thief {
    fn capabilities() -> Vec<Capability> {
        vec![Capability::Search]
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

    fn search(victim: String, _filters: SearchFilters, _page: PageRequest) -> Result<ItemPage> {
        let names = [
            "token".to_string(),
            format!("../{victim}/token"),
            format!("jellyfesh/{victim}/token"),
            format!("/jellyfesh/{victim}/token"),
            "~cookie-key".to_string(),
            format!("..%2F{victim}%2Ftoken"),
            format!("token\0{victim}"),
            "*".to_string(),
        ];
        let mut items = Vec::new();
        for (i, name) in names.iter().enumerate() {
            let result = kv::secret::get(name);
            items.push(Item::new(
                ItemKind::Track,
                format!("s{i}"),
                format!("secret {name:?} => {result:?}"),
            ));
        }
        for (i, key) in ["token", "greeting", &format!("../{victim}/greeting")]
            .iter()
            .enumerate()
        {
            let result = kv::get(key);
            items.push(Item::new(
                ItemKind::Track,
                format!("k{i}"),
                format!("kv {key:?} => {result:?}"),
            ));
        }
        let server = SERVER.with(|s| s.borrow().clone());
        let cookies = http::Request::get(format!("{server}/cookies"))
            .send()
            .map(|r| r.text())
            .unwrap_or_else(|e| format!("{e:?}"));
        items.push(Item::new(ItemKind::Track, "c", format!("cookies => {cookies:?}")));
        Ok(ItemPage::new(items))
    }
}

jf_plugin_sdk::export_plugin!(Thief);
