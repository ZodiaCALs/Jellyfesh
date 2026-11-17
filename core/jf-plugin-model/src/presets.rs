//! The music services the apps offer in Sources → Add a Source, and the plugins that connect to
//! them. Shared by the core and Jellyfesh Web, so every app lists the same services. A service is
//! offered only while its plugin is installed and signed by Jellyfesh: the list grows as plugins
//! for more services are ready.

/// A service in the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preset {
    /// Stable name, which the apps key the service's picture and description by.
    pub id: &'static str,
    /// The service's own name.
    pub name: &'static str,
    /// The official plugin that connects to it.
    pub plugin: &'static str,
    /// The plugin setting that holds the server's address, for self-hosted servers.
    pub address_setting: Option<&'static str>,
    /// An address shown as an example in that field.
    pub address_example: Option<&'static str>,
}

/// The services, in the order the apps list them.
pub const PRESETS: &[Preset] = &[
    Preset {
        id: "jellyfin",
        name: "Jellyfin",
        plugin: "org.jellyfesh.jellyfin",
        address_setting: Some("server_url"),
        address_example: Some("http://192.168.1.20:8096"),
    },
    Preset {
        id: "navidrome",
        name: "Navidrome",
        plugin: "org.jellyfesh.subsonic",
        address_setting: Some("server_url"),
        address_example: Some("http://192.168.1.20:4533"),
    },
    // Other servers that speak the Subsonic API: Airsonic, Gonic, Ampache and more.
    Preset {
        id: "subsonic",
        name: "Subsonic",
        plugin: "org.jellyfesh.subsonic",
        address_setting: Some("server_url"),
        address_example: Some("https://music.example.com"),
    },
    // Online services: no address; each person signs in with their own key or app (ADR-0018).
    // Metadata and links to QQ Music, which plays its music itself.
    Preset {
        id: "qqmusic",
        name: "QQ Music",
        plugin: "org.jellyfesh.qqmusic",
        address_setting: None,
        address_example: None,
    },
    // Remote control of the person's own Spotify devices (beta).
    Preset {
        id: "spotify",
        name: "Spotify",
        plugin: "org.jellyfesh.spotify",
        address_setting: None,
        address_example: None,
    },
    // NetEase Cloud Music (org.jellyfesh.netease) is not listed until it can search.
];

/// The service named `id`.
pub fn preset(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique_and_plugins_are_official_ids() {
        for (i, a) in PRESETS.iter().enumerate() {
            assert!(PRESETS[i + 1..].iter().all(|b| b.id != a.id), "{} twice", a.id);
            assert!(a.plugin.starts_with("org.jellyfesh."), "{}", a.plugin);
            assert!(crate::manifest::valid_setting_key(
                a.address_setting.unwrap_or("x")
            ));
            assert_eq!(
                a.address_setting.is_some(),
                a.address_example.is_some(),
                "{}",
                a.id
            );
        }
        assert_eq!(
            preset("navidrome").map(|p| p.plugin),
            Some("org.jellyfesh.subsonic")
        );
        assert!(preset("nothing").is_none());
        // Online services ask for no address.
        for id in ["qqmusic", "spotify"] {
            let p = preset(id).unwrap();
            assert_eq!((p.address_setting, p.address_example), (None, None), "{id}");
        }
        assert!(
            PRESETS.iter().all(|p| p.plugin != "org.jellyfesh.netease"),
            "NetEase cannot search yet"
        );
    }
}
