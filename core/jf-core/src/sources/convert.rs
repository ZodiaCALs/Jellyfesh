//! Between the plugin host's types (the WIT records) and the core API's.

use jf_library::SearchResults;
use jf_plugin_host::links::Links;
use jf_plugin_host::merge::MergedItem;
use jf_plugin_host::{
    AuthKind, AuthStep as HostStep, Capability, FieldKind, FieldSpec, ImageRef, Item, ItemKind as HostKind,
    LyricFormat, PlaybackMode, PluginInfo, SignatureStatus, StreamInfo, UnplayableReason, capability_name,
};
use jf_session::{SourceTrack, TrackRef};

use crate::types::{
    ArtworkRef, AuthField, AuthFieldKind, AuthMethod, AuthStep, ItemKind, PluginBadge, PluginKind,
    PluginSummary, SourceItem, SourcePreset,
};

/// The `source` of library results.
pub(crate) const LIBRARY: &str = "library";

pub(crate) fn kind(kind: HostKind) -> ItemKind {
    match kind {
        HostKind::Track => ItemKind::Track,
        HostKind::Album => ItemKind::Album,
        HostKind::Artist => ItemKind::Artist,
        HostKind::Playlist => ItemKind::Playlist,
        HostKind::Folder => ItemKind::Folder,
    }
}

pub(crate) fn auth_kind(method: AuthMethod) -> AuthKind {
    match method {
        AuthMethod::Qr => AuthKind::Qr,
        AuthMethod::Oauth => AuthKind::Oauth,
        AuthMethod::CookieImport => AuthKind::CookieImport,
        AuthMethod::PasswordExchange => AuthKind::PasswordExchange,
        AuthMethod::ApiKey => AuthKind::ApiKey,
    }
}

pub(crate) fn auth_method(kind: AuthKind) -> AuthMethod {
    match kind {
        AuthKind::Qr => AuthMethod::Qr,
        AuthKind::Oauth => AuthMethod::Oauth,
        AuthKind::CookieImport => AuthMethod::CookieImport,
        AuthKind::PasswordExchange => AuthMethod::PasswordExchange,
        AuthKind::ApiKey => AuthMethod::ApiKey,
    }
}

/// Why the service will not play an item here, in the service's words when
/// it gave some. `None` for playable items.
pub(crate) fn unplayable_reason(item: &Item) -> Option<String> {
    if item.playable {
        return None;
    }
    let unplayable = item.unplayable.as_ref();
    if let Some(message) = unplayable
        .and_then(|u| u.message.as_deref())
        .filter(|m| !m.trim().is_empty())
    {
        return Some(message.to_owned());
    }
    Some(
        match unplayable.map(|u| u.reason) {
            Some(UnplayableReason::Paywall) => "Needs a subscription or a purchase",
            Some(UnplayableReason::Region) => "Not available in your region",
            Some(UnplayableReason::Unavailable) => "Not available",
            Some(UnplayableReason::Other) | None => "Not playable here",
        }
        .to_owned(),
    )
}

/// What a source track keeps in the queue (and in playlists).
pub(crate) fn source_track(plugin: &str, item: &Item) -> SourceTrack {
    SourceTrack {
        plugin: plugin.to_owned(),
        id: item.id.clone(),
        title: item.title.clone(),
        artist: item.artist.clone(),
        album: item.album.clone(),
        duration_ms: item.duration_ms.map(u64::from),
        artwork_url: item.artwork.as_ref().map(|a| a.url.clone()),
        isrc: item.isrc.clone(),
    }
}

/// A plugin's item as a list row; `links` are the plugin's (ADR-0018).
pub(crate) fn source_item(plugin: &str, item: &Item, links: &Links) -> SourceItem {
    let artwork = crate::artwork::row_picture(plugin, item.kind, &item.id, item.artwork.as_ref());
    let track =
        (item.kind == HostKind::Track && item.playable).then(|| TrackRef::Source(source_track(plugin, item)));
    SourceItem {
        source: plugin.to_owned(),
        kind: kind(item.kind),
        id: item.id.clone(),
        title: item.title.clone(),
        subtitle: item.artist.clone(),
        album: item.album.clone(),
        duration_ms: item.duration_ms.map(u64::from),
        artwork: artwork.map(ArtworkRef::Url),
        playable: item.playable,
        unplayable_reason: unplayable_reason(item),
        track,
        also_in: Vec::new(),
        link: links.url_for(item.kind, &item.id),
        artist_id: item.artist_id.clone(),
        album_id: item.album_id.clone(),
        downloaded: false,
    }
}

/// An item for a source track the core has no listing for (a restored
/// queue, a playlist): what the plugin gave when it was listed.
pub(crate) fn track_item(track: &SourceTrack) -> Item {
    let mut item = bare(&track.id, HostKind::Track, &track.title);
    item.artist.clone_from(&track.artist);
    item.album.clone_from(&track.album);
    item.duration_ms = track.duration_ms.map(|d| u32::try_from(d).unwrap_or(u32::MAX));
    item.isrc.clone_from(&track.isrc);
    item.artwork = track.artwork_url.as_ref().map(|url| image(url));
    item
}

/// An item that only has an id, a kind and a title.
pub(crate) fn bare(id: &str, kind: HostKind, title: &str) -> Item {
    Item {
        id: id.to_owned(),
        kind,
        title: title.to_owned(),
        artist: None,
        album: None,
        album_id: None,
        artist_id: None,
        duration_ms: None,
        track_number: None,
        disc_number: None,
        year: None,
        child_count: None,
        isrc: None,
        artwork: None,
        explicit: false,
        playable: true,
        unplayable: None,
    }
}

/// A picture reference without headers.
pub(crate) fn image(url: &str) -> ImageRef {
    ImageRef {
        url: url.to_owned(),
        headers: Vec::new(),
        width: None,
        height: None,
    }
}

/// Library search results as items (for merging) and rows, in the order
/// tracks, albums, artists.
pub(crate) fn library_answer(results: &SearchResults) -> (Vec<Item>, Vec<SourceItem>) {
    let mut items = Vec::new();
    let mut rows = Vec::new();
    for t in &results.tracks {
        let mut item = bare(&t.id.to_string(), HostKind::Track, &t.title);
        item.artist.clone_from(&t.artist);
        item.album.clone_from(&t.album);
        item.duration_ms = t.duration_ms.map(|d| u32::try_from(d).unwrap_or(u32::MAX));
        item.isrc.clone_from(&t.isrc);
        item.year = t.year.and_then(|y| u32::try_from(y).ok());
        items.push(item);
        rows.push(SourceItem {
            source: LIBRARY.into(),
            kind: ItemKind::Track,
            id: t.id.to_string(),
            title: t.title.clone(),
            subtitle: t.artist.clone(),
            album: t.album.clone(),
            duration_ms: t.duration_ms,
            artwork: None,
            playable: true,
            unplayable_reason: None,
            track: Some(TrackRef::Library(t.id)),
            also_in: Vec::new(),
            link: None,
            artist_id: None,
            album_id: None,
            downloaded: false,
        });
    }
    for a in &results.albums {
        let mut item = bare(&a.id.to_string(), HostKind::Album, &a.title);
        item.artist.clone_from(&a.album_artist);
        items.push(item);
        rows.push(SourceItem {
            source: LIBRARY.into(),
            kind: ItemKind::Album,
            id: a.id.to_string(),
            title: a.title.clone(),
            subtitle: a.album_artist.clone(),
            album: None,
            duration_ms: None,
            artwork: None,
            playable: true,
            unplayable_reason: None,
            track: None,
            also_in: Vec::new(),
            link: None,
            artist_id: None,
            album_id: None,
            downloaded: false,
        });
    }
    for a in &results.artists {
        items.push(bare(&a.id.to_string(), HostKind::Artist, &a.name));
        rows.push(SourceItem {
            source: LIBRARY.into(),
            kind: ItemKind::Artist,
            id: a.id.to_string(),
            title: a.name.clone(),
            subtitle: None,
            album: None,
            duration_ms: None,
            artwork: None,
            playable: true,
            unplayable_reason: None,
            track: None,
            also_in: Vec::new(),
            link: None,
            artist_id: None,
            album_id: None,
            downloaded: false,
        });
    }
    (items, rows)
}

/// What one source answered to a search: its items, and the row for each.
#[derive(Debug, Clone)]
pub(crate) struct Answer {
    pub source: String,
    pub items: Vec<Item>,
    pub rows: Vec<SourceItem>,
}

impl Answer {
    /// A plugin's answer; `links` are the plugin's.
    pub(crate) fn plugin(plugin: &str, items: Vec<Item>, links: &Links) -> Answer {
        let rows = items.iter().map(|i| source_item(plugin, i, links)).collect();
        Answer {
            source: plugin.to_owned(),
            items,
            rows,
        }
    }
}

/// Merges the answers so far (in the order they came, the library first)
/// with [`jf_plugin_host::merge::merge`] (the same ISRC, or the same
/// normalised title, primary artist and, for tracks, duration within 2 s,
/// is one entry) and sorts the entries into one section per answer, in the
/// answers' order.
///
/// An entry is shown by its first playable copy in answer order (else its
/// first copy), so the library's copy wins when it has one: it goes into
/// that copy's section, at the copy's place in its source's own order, and
/// lists the other sources that have it in `also_in`. Empty fields are
/// filled from the other copies. A section therefore holds what its source
/// adds to the sources before it.
pub(crate) fn sections(answers: &[Answer]) -> Vec<Vec<SourceItem>> {
    let lists: Vec<(&str, &[Item])> = answers
        .iter()
        .map(|a| (a.source.as_str(), a.items.as_slice()))
        .collect();
    // Which of each answer's items an entry already took (a source may list
    // the same item twice; each copy is one row).
    let mut used: Vec<Vec<bool>> = answers.iter().map(|a| vec![false; a.items.len()]).collect();
    let mut sections: Vec<Vec<(usize, SourceItem)>> = vec![Vec::new(); answers.len()];
    for merged in jf_plugin_host::merge::merge(&lists) {
        if let Some((answer, index, row)) = entry(answers, &mut used, &merged) {
            sections[answer].push((index, row));
        }
    }
    sections
        .into_iter()
        .map(|mut section| {
            section.sort_by_key(|(index, _)| *index);
            section.into_iter().map(|(_, row)| row).collect()
        })
        .collect()
}

/// The row that shows a merged entry: (its answer, its index there, the row).
fn entry(
    answers: &[Answer],
    used: &mut [Vec<bool>],
    merged: &MergedItem,
) -> Option<(usize, usize, SourceItem)> {
    // Every copy as (answer, index, playable), in answer order.
    let mut copies: Vec<(usize, usize, bool)> = Vec::new();
    for copy in &merged.sources {
        let Some(answer) = answers.iter().position(|a| a.source == copy.source) else {
            continue;
        };
        let found =
            answers[answer].items.iter().enumerate().position(|(i, item)| {
                !used[answer][i] && item.id == copy.id && item.kind == merged.item.kind
            });
        if let Some(index) = found {
            used[answer][index] = true;
            copies.push((answer, index, copy.playable));
        }
    }
    copies.sort_by_key(|(answer, _, _)| *answer);
    let &(answer, index, _) = copies.iter().find(|c| c.2).or_else(|| copies.first())?;
    let mut row = answers[answer].rows.get(index)?.clone();
    for &(other, _, _) in &copies {
        let source = &answers[other].source;
        if other != answer && !row.also_in.contains(source) {
            row.also_in.push(source.clone());
        }
    }
    if row.album.is_none() {
        row.album.clone_from(&merged.item.album);
    }
    if row.duration_ms.is_none() {
        row.duration_ms = merged.item.duration_ms.map(u64::from);
    }
    if row.artwork.is_none() {
        row.artwork = merged
            .item
            .artwork
            .as_ref()
            .map(|a| ArtworkRef::Url(a.url.clone()));
    }
    Some((answer, index, row))
}

/// A plugin as the `Sources` slice shows it.
/// The services of `jf_plugin_model::presets` whose official plugin is
/// installed, in the list's order.
pub(crate) fn presets(plugins: &[PluginSummary]) -> Vec<SourcePreset> {
    jf_plugin_host::presets::PRESETS
        .iter()
        .filter_map(|preset| {
            let plugin = plugins
                .iter()
                .find(|p| p.id == preset.plugin && p.badge == PluginBadge::Official)?;
            let address = preset.address_setting.and_then(|key| {
                let settings: serde_json::Value = serde_json::from_str(&plugin.settings_json).ok()?;
                let address = settings.get(key)?.as_str()?.trim();
                (!address.is_empty()).then(|| address.to_owned())
            });
            Some(SourcePreset {
                id: preset.id.to_owned(),
                name: preset.name.to_owned(),
                plugin: plugin.id.clone(),
                auth_methods: plugin.auth_methods.clone(),
                needs_address: preset.address_setting.is_some(),
                address,
                address_example: preset.address_example.map(str::to_owned),
                account: plugin.account.clone(),
            })
        })
        .collect()
}

pub(crate) fn summary(
    info: &PluginInfo,
    allow_community: bool,
    acknowledged: bool,
    icon: Option<std::path::PathBuf>,
) -> PluginSummary {
    let blocked = !info.official && !allow_community;
    let disabled_reason = if blocked {
        Some("Community plugins are turned off".to_owned())
    } else if info.enabled {
        None
    } else {
        info.disabled_reason.clone()
    };
    PluginSummary {
        id: info.id.clone(),
        name: info.name.clone(),
        version: info.version.clone(),
        author: info.author.clone(),
        description: info.description.clone(),
        badge: if info.official {
            PluginBadge::Official
        } else {
            PluginBadge::Community
        },
        signing_key: match &info.signature {
            SignatureStatus::Community { key_id } => Some(key_id.clone()),
            SignatureStatus::Unsigned | SignatureStatus::Trusted { .. } => None,
        },
        vendor_playback: matches!(info.playback_mode, PlaybackMode::Vendor | PlaybackMode::Remote),
        remote_playback: info.playback_mode == PlaybackMode::Remote,
        capabilities: info
            .capabilities
            .iter()
            .map(|c| capability_name(*c).to_owned())
            .collect(),
        auth_methods: info.auth_kinds.iter().map(|k| auth_method(*k)).collect(),
        enabled: info.enabled && !blocked,
        disabled_reason,
        account: info.account.clone(),
        needs_disclosure: !info.official && !acknowledged,
        attribution: info.content_policy.attribution.clone(),
        // The text keeps the plugin's order of settings; the parsed value sorts them by key.
        settings_schema_json: if info.settings_schema_text.is_empty() {
            info.settings_schema.to_string()
        } else {
            info.settings_schema_text.clone()
        },
        settings_json: info.settings.to_string(),
        icon,
        kind: if jf_plugin_host::is_extension(&info.capabilities) {
            PluginKind::Extension
        } else {
            PluginKind::Source
        },
        requires_pro: jf_plugin_host::requires_pro(&info.capabilities),
        permissions: info.permissions.iter().map(ToString::to_string).collect(),
        discover: info.capabilities.contains(&Capability::Discover),
        details: info.capabilities.contains(&Capability::Details),
        comments: info.capabilities.contains(&Capability::Comments),
        library: info.capabilities.contains(&Capability::Library),
        download: info.capabilities.contains(&Capability::Download),
        qualities: info
            .content_policy
            .qualities
            .iter()
            .map(|q| streaming_quality(*q))
            .collect(),
        local_network: server_is_local(info),
    }
}

/// The core's level for a plugin's quality.
pub(crate) fn streaming_quality(quality: jf_plugin_host::Quality) -> crate::types::StreamingQuality {
    use crate::types::StreamingQuality;
    match quality {
        jf_plugin_host::Quality::Lossless => StreamingQuality::Lossless,
        jf_plugin_host::Quality::High => StreamingQuality::High,
        jf_plugin_host::Quality::Normal => StreamingQuality::Normal,
        jf_plugin_host::Quality::Low => StreamingQuality::Low,
    }
}

/// Whether a server address the person set for the plugin (a setting its
/// allowlist names, `${settings.server_url}`) is on the local network.
pub(crate) fn server_is_local(info: &PluginInfo) -> bool {
    info.server_settings.iter().any(|key| {
        info.settings
            .get(key)
            .and_then(serde_json::Value::as_str)
            .is_some_and(crate::network::address_is_local)
    })
}

/// A login step as the UI renders it.
pub(crate) fn auth_step(step: HostStep) -> AuthStep {
    let at = |ms: Option<u64>| ms.map(|ms| i64::try_from(ms).unwrap_or(i64::MAX));
    match step {
        HostStep::ShowQr {
            data,
            hint,
            expires_at_ms,
        } => AuthStep::ShowQr {
            data,
            hint,
            expires_at_ms: at(expires_at_ms),
        },
        HostStep::ShowCode {
            code,
            hint,
            expires_at_ms,
        } => AuthStep::ShowCode {
            code,
            hint,
            expires_at_ms: at(expires_at_ms),
        },
        HostStep::OpenUrl { url } => AuthStep::OpenUrl { url },
        HostStep::AskFields { fields } => AuthStep::AskFields {
            fields: fields.into_iter().map(field).collect(),
        },
        HostStep::Waiting { message } => AuthStep::Waiting { message },
        HostStep::Done { account } => AuthStep::Done { account },
        HostStep::Failed { message } => AuthStep::Failed { message },
    }
}

fn field(spec: FieldSpec) -> AuthField {
    AuthField {
        name: spec.name,
        label: spec.label,
        kind: match spec.kind {
            FieldKind::Text => AuthFieldKind::Text,
            FieldKind::Email => AuthFieldKind::Email,
            FieldKind::Url => AuthFieldKind::Url,
            FieldKind::Number => AuthFieldKind::Number,
            FieldKind::CookieHeader => AuthFieldKind::CookieHeader,
        },
        secret: spec.secret,
        required: spec.required,
        placeholder: spec.placeholder,
        help: spec.help,
    }
}

/// The parser for a plugin's lyrics, or `None` for a format the core does
/// not read (JSON syllables).
pub(crate) fn lyrics_format(format: LyricFormat) -> Option<jf_lyrics::Format> {
    match format {
        LyricFormat::Plain => Some(jf_lyrics::Format::Plain),
        LyricFormat::Lrc => Some(jf_lyrics::Format::Lrc),
        LyricFormat::Elrc => Some(jf_lyrics::Format::EnhancedLrc),
        LyricFormat::Ttml => Some(jf_lyrics::Format::Ttml),
        LyricFormat::Vtt => Some(jf_lyrics::Format::WebVtt),
        LyricFormat::JsonSyllables => None,
    }
}

/// A resolved stream as the engine fetches it.
pub(crate) fn http_source(info: StreamInfo) -> jf_engine::HttpSource {
    jf_engine::HttpSource {
        url: info.url,
        headers: info.headers.into_iter().map(|h| (h.name, h.value)).collect(),
        expires_at_ms: info.expires_at_ms.map(|ms| i64::try_from(ms).unwrap_or(i64::MAX)),
        size: None,
        // The server's content type (and the URL's extension) tell the
        // demuxer more reliably than the plugin's codec hint.
        mime: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jf_library::{Album, Artist, ReplayGain, Track};

    /// A plugin's answer, without links.
    fn answer(plugin: &str, items: Vec<Item>) -> Answer {
        Answer::plugin(plugin, items, &Links::default())
    }

    fn track(id: &str, title: &str, artist: &str, duration_ms: u32, isrc: Option<&str>) -> Item {
        let mut item = bare(id, HostKind::Track, title);
        item.artist = Some(artist.into());
        item.duration_ms = Some(duration_ms);
        item.isrc = isrc.map(str::to_owned);
        item
    }

    fn library_track(id: i64, title: &str, artist: &str, duration_ms: u64, isrc: Option<&str>) -> Track {
        Track {
            id,
            path: format!("/music/{title}.flac").into(),
            title: title.into(),
            artist: Some(artist.into()),
            album: Some("Album".into()),
            album_artist: None,
            album_id: Some(1),
            track_no: None,
            disc_no: None,
            year: None,
            genre: None,
            duration_ms: Some(duration_ms),
            sample_rate: None,
            channels: None,
            bit_depth: None,
            codec: None,
            replaygain: ReplayGain::default(),
            isrc: isrc.map(str::to_owned),
            has_embedded_lyrics: false,
            artwork_hash: None,
            file_size: 0,
            mtime_ns: 0,
        }
    }

    fn library(tracks: Vec<Track>) -> Answer {
        let results = SearchResults {
            tracks,
            albums: vec![Album {
                id: 1,
                title: "Album".into(),
                album_artist: Some("Someone".into()),
                year: None,
                track_count: 2,
                artwork_hash: None,
            }],
            artists: vec![Artist {
                id: 1,
                name: "Someone".into(),
                track_count: 2,
            }],
        };
        let (items, rows) = library_answer(&results);
        Answer {
            source: LIBRARY.into(),
            items,
            rows,
        }
    }

    fn tracks_of(section: &[SourceItem]) -> Vec<&SourceItem> {
        section.iter().filter(|r| r.kind == ItemKind::Track).collect()
    }

    #[test]
    fn library_tracks_win_and_list_the_sources_that_also_have_them() {
        let lib = library(vec![
            library_track(7, "Blue Hour", "Someone", 200_000, None),
            library_track(8, "Other Song", "Someone", 150_000, Some("USRC17607839")),
        ]);
        let a = answer(
            "org.example.a",
            vec![
                // Same title and primary artist, within 2 s: the library's track.
                track("a1", "Blue Hour (Remastered)", "Someone feat. X", 201_500, None),
                // Same ISRC, different title: still the library's track.
                track(
                    "a2",
                    "Other Song - Live",
                    "Someone Else",
                    300_000,
                    Some("us-rc1-76-07839"),
                ),
                track("a3", "Only Here", "Someone", 100_000, None),
            ],
        );
        let sections = sections(&[lib, a]);
        assert_eq!(sections.len(), 2);
        let library = tracks_of(&sections[0]);
        assert_eq!(library.len(), 2, "{sections:#?}");
        assert!(library.iter().all(|r| r.source == LIBRARY));
        assert_eq!(library[0].track, Some(TrackRef::Library(7)));
        assert_eq!(library[0].also_in, vec!["org.example.a".to_string()]);
        assert_eq!(library[1].track, Some(TrackRef::Library(8)));
        assert_eq!(library[1].also_in, vec!["org.example.a".to_string()]);
        // The plugin's section holds what the library does not have.
        let plugin = tracks_of(&sections[1]);
        assert_eq!(plugin.len(), 1, "{sections:#?}");
        assert_eq!(plugin[0].source, "org.example.a");
        assert!(plugin[0].also_in.is_empty());
        assert!(matches!(&plugin[0].track, Some(TrackRef::Source(t)) if t.id == "a3"));
        // Albums and artists stay in the library's section; kinds never mix.
        assert_eq!(
            sections[0].iter().filter(|r| r.kind == ItemKind::Album).count(),
            1
        );
        assert_eq!(
            sections[0].iter().filter(|r| r.kind == ItemKind::Artist).count(),
            1
        );
        assert_eq!(sections[1].len(), 1);
    }

    #[test]
    fn a_library_copy_wins_wherever_it_ranks_and_keeps_the_library_order() {
        let lib = library(vec![
            library_track(1, "One", "Someone", 100_000, None),
            library_track(2, "Two", "Someone", 110_000, None),
            library_track(3, "Blue Hour", "Someone", 200_000, None),
        ]);
        // The plugin's best hit is the library's third track.
        let a = answer("p", vec![track("x", "Blue Hour", "Someone", 200_500, None)]);
        let sections = sections(&[lib, a]);
        let titles: Vec<&str> = tracks_of(&sections[0]).iter().map(|r| r.title.as_str()).collect();
        assert_eq!(titles, vec!["One", "Two", "Blue Hour"]);
        assert_eq!(tracks_of(&sections[0])[2].also_in, vec!["p".to_string()]);
        assert!(sections[1].is_empty());
    }

    #[test]
    fn the_library_alone_is_its_own_rows() {
        let lib = library(vec![
            library_track(7, "Blue Hour", "Someone", 200_000, None),
            library_track(8, "Other Song", "Someone", 150_000, None),
        ]);
        let rows = lib.rows.clone();
        assert_eq!(sections(&[lib]), vec![rows]);
    }

    #[test]
    fn a_take_three_seconds_apart_is_another_recording() {
        let lib = library(vec![library_track(7, "Song", "Someone", 200_000, None)]);
        let a = answer("p", vec![track("x", "Song", "Someone", 203_100, None)]);
        let sections = sections(&[lib, a]);
        assert_eq!(tracks_of(&sections[0]).len(), 1);
        assert_eq!(tracks_of(&sections[1]).len(), 1);
    }

    #[test]
    fn an_unplayable_copy_gives_way_to_a_playable_one_and_keeps_its_reason() {
        let mut locked = track("x", "Song", "Someone", 200_000, Some("USRC17607839"));
        locked.playable = false;
        locked.unplayable = Some(jf_plugin_host::Unplayable {
            reason: UnplayableReason::Paywall,
            message: None,
        });
        let open = track("y", "Song", "Someone", 200_400, Some("USRC17607839"));
        let both = sections(&[answer("a", vec![locked.clone()]), answer("b", vec![open])]);
        assert!(both[0].is_empty(), "{both:#?}");
        assert_eq!(both[1].len(), 1);
        assert_eq!((both[1][0].source.as_str(), both[1][0].id.as_str()), ("b", "y"));
        assert_eq!(both[1][0].also_in, vec!["a".to_string()]);
        assert!(both[1][0].playable && both[1][0].track.is_some());

        let alone = sections(&[answer("a", vec![locked])]);
        assert!(!alone[0][0].playable);
        assert_eq!(
            alone[0][0].track, None,
            "an unplayable item never reaches the queue"
        );
        assert_eq!(
            alone[0][0].unplayable_reason.as_deref(),
            Some("Needs a subscription or a purchase")
        );
    }

    #[test]
    fn an_item_listed_twice_by_one_source_is_two_rows() {
        let a = answer(
            "p",
            vec![
                track("x", "Song", "Someone", 200_000, None),
                track("x", "Song", "Someone", 200_000, None),
            ],
        );
        assert_eq!(sections(&[a])[0].len(), 2);
    }

    #[test]
    fn source_tracks_round_trip_to_items() {
        let mut item = track("t1", "Song", "Someone", 200_000, Some("USRC17607839"));
        item.album = Some("Album".into());
        item.artwork = Some(image("http://server/art/t1"));
        let t = source_track("p", &item);
        assert_eq!(t.duration_ms, Some(200_000));
        assert_eq!(t.artwork_url.as_deref(), Some("http://server/art/t1"));
        let back = track_item(&t);
        assert_eq!(
            (back.id.as_str(), back.kind, back.title.as_str()),
            ("t1", HostKind::Track, "Song")
        );
        assert_eq!(back.isrc, item.isrc);
        assert_eq!(back.album, item.album);
        assert_eq!(back.artwork.map(|a| a.url), Some("http://server/art/t1".into()));
    }
}
