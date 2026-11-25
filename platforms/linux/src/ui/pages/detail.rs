//! Album and artist pages: a header with artwork and actions, then the
//! songs (and, for artists, their albums). After Apple Music
//! (design-controls.md, "Album page"): large artwork (240 px, the full width
//! on phones), the title, the artist in accent text, genre, year and song
//! count, Play (accent) and Shuffle (glass), and a footer with the songs and
//! minutes.

use std::collections::HashSet;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use jf_core::{Collection, Track, TrackQuery, TrackSort};

use crate::logic::{format, library};
use crate::ui::convert;
use crate::ui::i18n::{tr, trf, trn};
use crate::ui::models::{Item, ItemData, Kind};
use crate::ui::pages::{frame, heading, pill, scroller};
use crate::ui::rows::{Card, ListRow, RowStyle};
use crate::ui::state::{Change, app, spawn};
use crate::ui::thumbs::ArtKey;
use crate::ui::widgets::art::Art;
use crate::ui::widgets::glyph;
use crate::ui::widgets::maxwidth::MaxWidth;
use crate::ui::{actions, window};

/// The artwork's side on album, artist and playlist pages (desktop).
const HERO_ART: i32 = 240;

/// The header shared by album, artist and playlist pages.
pub struct Header {
    pub root: gtk::Box,
    pub art: Art,
    pub texts: gtk::Box,
    pub avatar: gtk::Label,
    pub title: gtk::Label,
    pub subtitle: gtk::Button,
    pub meta: gtk::Label,
    pub play: gtk::Button,
    pub shuffle: gtk::Button,
    pub more: gtk::MenuButton,
}

impl Header {
    pub fn new(kind: &str, round: bool) -> Header {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 28);
        root.add_css_class("detail-header");
        let art = Art::new(HERO_ART, jf_tokens::artwork::RADIUS_HERO);
        art.set_size_request(HERO_ART, HERO_ART);
        art.set_valign(gtk::Align::Start);
        art.add_css_class("hero-art");
        let avatar = gtk::Label::new(None);
        avatar.add_css_class("avatar");
        avatar.add_css_class("avatar-large");
        avatar.set_size_request(168, 168);
        avatar.set_valign(gtk::Align::Start);
        if round {
            root.append(&avatar);
        } else {
            root.append(&art);
        }
        let texts = gtk::Box::new(gtk::Orientation::Vertical, 6);
        texts.set_valign(gtk::Align::Center);
        texts.set_hexpand(true);
        let kind_label = gtk::Label::new(Some(kind));
        kind_label.set_xalign(0.0);
        kind_label.add_css_class("detail-kind");
        let title = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .accessible_role(gtk::AccessibleRole::Heading)
            .build();
        title.add_css_class("detail-title");
        let subtitle = gtk::Button::builder().halign(gtk::Align::Start).build();
        subtitle.add_css_class("flat");
        subtitle.add_css_class("detail-subtitle");
        let meta = gtk::Label::new(None);
        meta.set_xalign(0.0);
        meta.add_css_class("detail-meta");
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        buttons.add_css_class("detail-buttons");
        let play = pill(&tr("Play"), "play", true);
        let shuffle = pill(&tr("Shuffle"), "shuffle", false);
        shuffle.add_css_class("glass-pill");
        let more = gtk::MenuButton::builder()
            .child(&glyph::Glyph::new("more", 20))
            .tooltip_text(tr("More"))
            .build();
        more.add_css_class("circular");
        more.add_css_class("flat");
        more.update_property(&[gtk::accessible::Property::Label(&tr("More"))]);
        buttons.append(&play);
        buttons.append(&shuffle);
        buttons.append(&more);
        texts.append(&kind_label);
        texts.append(&title);
        texts.append(&subtitle);
        texts.append(&meta);
        texts.append(&buttons);
        root.append(&texts);
        let header = Header {
            root,
            art,
            texts,
            avatar,
            title,
            subtitle,
            meta,
            play,
            shuffle,
            more,
        };
        header.follow_width();
        header
    }

    /// Phones (the phone layout, up to 760 px): the artwork above the text
    /// at the full width; wider windows: side by side at 240 px.
    fn follow_width(&self) {
        let (root, art, texts) = (
            self.root.downgrade(),
            self.art.downgrade(),
            self.texts.downgrade(),
        );
        let apply = move || {
            let (Some(root), Some(art), Some(texts)) = (root.upgrade(), art.upgrade(), texts.upgrade())
            else {
                return;
            };
            if window::is_phone() {
                root.set_orientation(gtk::Orientation::Vertical);
                root.add_css_class("narrow");
                art.set_size_request(-1, -1);
                art.set_fixed(false);
                art.set_halign(gtk::Align::Fill);
                texts.set_halign(gtk::Align::Fill);
            } else {
                root.set_orientation(gtk::Orientation::Horizontal);
                root.remove_css_class("narrow");
                art.set_size_request(HERO_ART, HERO_ART);
                art.set_fixed(true);
                art.set_halign(gtk::Align::Start);
            }
        };
        apply();
        app().listen_for(&self.root, move |c| {
            if matches!(c, Change::Layout) {
                apply();
            }
        });
    }
}

/// A list of track rows; activating one calls `on_activate(index)`.
pub fn track_box(style: RowStyle, on_activate: impl Fn(usize) + 'static) -> gtk::ListBox {
    let list = gtk::ListBox::new();
    list.add_css_class("track-box");
    list.set_selection_mode(gtk::SelectionMode::None);
    list.connect_row_activated(move |_, row| {
        if let Ok(i) = usize::try_from(row.index()) {
            on_activate(i);
        }
    });
    let _ = style;
    list
}

pub fn fill_track_box(list: &gtk::ListBox, style: RowStyle, rows: &[ItemData]) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    for data in rows {
        let row = ListRow::new(style);
        row.bind(&Item::new(Some(data.clone())));
        list.append(&gtk::ListBoxRow::builder().child(&row).activatable(true).build());
    }
}

/// A detail page's column: at most 1,000 px wide, lined up with the page's
/// start, as on the web.
fn page_body(header: &Header) -> (gtk::Box, MaxWidth) {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 20);
    body.add_css_class("detail");
    body.append(&header.root);
    let column = MaxWidth::new(&body, 1000);
    (body, column)
}

/// Genre, year and song count under an album's artist.
fn album_meta(tracks: &[Track]) -> String {
    let year = tracks.iter().filter_map(|t| t.year).min().map(|y| y.to_string());
    let genre = library::main_genre(tracks.iter().map(|t| t.genre.as_deref()));
    let songs = trn("{n} song", "{n} songs", tracks.len() as u64);
    format::dot_join([genre.as_deref(), year.as_deref(), Some(songs.as_str())])
}

/// "12 songs, 48 minutes" under the songs.
pub fn songs_and_minutes(durations: impl IntoIterator<Item = Option<u64>>) -> String {
    let (songs, minutes) = library::totals(durations);
    trf(
        "{songs}, {minutes}",
        &[
            ("songs", &trn("{n} song", "{n} songs", songs)),
            ("minutes", &trn("{n} minute", "{n} minutes", minutes)),
        ],
    )
}

/// The footer label under a list of songs.
fn footer() -> gtk::Label {
    let l = gtk::Label::builder().xalign(0.0).build();
    l.add_css_class("detail-footer");
    l.add_css_class("numeric");
    l
}

pub fn album_page(id: i64, known: Option<ItemData>) -> adw::NavigationPage {
    let header = Header::new(&tr("Album"), false);
    let collection = Collection::Album { id };
    header.play.connect_clicked(glib::clone!(
        #[strong]
        collection,
        move |_| actions::play(collection.clone(), 0, false)
    ));
    header.shuffle.connect_clicked(glib::clone!(
        #[strong]
        collection,
        move |_| actions::play(collection.clone(), 0, true)
    ));
    let menu_data = ItemData {
        kind: Kind::Album,
        id,
        ..known.clone().unwrap_or_default()
    };
    header
        .more
        .set_menu_model(Some(&actions::more_menu_for(&menu_data)));
    if let Some(k) = &known {
        header.title.set_label(&k.title);
        header.subtitle.set_label(&k.subtitle);
        header.art.set_art(k.art.clone());
    }
    let tracks = track_box(RowStyle::ALBUM_TRACK, move |i| {
        actions::play(Collection::Album { id }, i, false);
    });
    let (body, clamp) = page_body(&header);
    body.append(&tracks);
    let foot = footer();
    body.append(&foot);
    let title = known.as_ref().map_or_else(|| tr("Album"), |k| k.title.clone());
    let (page, _) = frame(&title, &format!("album-{id}"), &scroller(&clamp));

    // The artist link opens the album artist's page.
    let artist_link = Rc::new(std::cell::RefCell::new(String::new()));
    header.subtitle.connect_clicked(glib::clone!(
        #[strong]
        artist_link,
        move |_| {
            let name = artist_link.borrow().clone();
            if !name.is_empty() {
                window::show_artist_named(&name);
            }
        }
    ));
    let header = Rc::new(header);
    let load = {
        let artist_link = artist_link.clone();
        let header = Rc::downgrade(&header);
        let tracks = tracks.downgrade();
        let page = page.downgrade();
        let foot = foot.downgrade();
        move || {
            let header = header.clone();
            let tracks = tracks.clone();
            let page = page.clone();
            let foot = foot.clone();
            let artist_link = artist_link.clone();
            spawn(async move {
                let result = app()
                    .query(move |core| {
                        core.album_tracks(id).map(|tracks| {
                            let favorites: Vec<bool> = tracks
                                .iter()
                                .map(|t| {
                                    core.is_favorite(&jf_core::TrackRef::Library(t.id))
                                        .unwrap_or(false)
                                })
                                .collect();
                            (tracks, favorites)
                        })
                    })
                    .await;
                let (Some(header), Some(list), Some(page)) =
                    (header.upgrade(), tracks.upgrade(), page.upgrade())
                else {
                    return;
                };
                let Some(Ok((album, favorites))) = result else {
                    return;
                };
                let Some(first) = album.first() else {
                    header
                        .meta
                        .set_label(&tr("This album has no songs left in your library."));
                    return;
                };
                let title = first.album.clone().unwrap_or_else(convert::unknown_album);
                let artist = first
                    .album_artist
                    .clone()
                    .or_else(|| first.artist.clone())
                    .unwrap_or_else(convert::unknown_artist);
                header.title.set_label(&title);
                header.subtitle.set_label(&artist);
                page.set_title(&title);
                header.meta.set_label(&album_meta(&album));
                if let Some(foot) = foot.upgrade() {
                    foot.set_label(&songs_and_minutes(album.iter().map(|t| t.duration_ms)));
                }
                let art = album
                    .iter()
                    .find_map(|t| t.artwork_hash.clone())
                    .map(ArtKey::Hash);
                header.art.set_art(art);
                *artist_link.borrow_mut() = artist.clone();
                let various = album.iter().any(|t| t.artist != first.artist);
                let rows: Vec<ItemData> = album
                    .iter()
                    .zip(favorites)
                    .map(|(t, favorite)| {
                        let mut d = convert::track(t);
                        d.favorite = favorite;
                        // On an album page the album is known: show the
                        // artist only when it varies.
                        d.subtitle = if various {
                            t.artist.clone().unwrap_or_default()
                        } else {
                            String::new()
                        };
                        d.art = None;
                        d
                    })
                    .collect();
                fill_track_box(&list, RowStyle::ALBUM_TRACK, &rows);
            });
        }
    };
    load();
    app().listen_for(&page, move |change| {
        if matches!(change, Change::Library) {
            load();
        }
    });
    crate::ui::pages::keep_alive(&page, header);
    page
}

pub fn artist_page(id: i64, known: Option<ItemData>) -> adw::NavigationPage {
    let header = Header::new(&tr("Artist"), true);
    let collection = Collection::Artist { id };
    header.play.connect_clicked(glib::clone!(
        #[strong]
        collection,
        move |_| actions::play(collection.clone(), 0, false)
    ));
    header.shuffle.connect_clicked(glib::clone!(
        #[strong]
        collection,
        move |_| actions::play(collection.clone(), 0, true)
    ));
    let menu_data = ItemData {
        kind: Kind::Artist,
        id,
        ..known.clone().unwrap_or_default()
    };
    header
        .more
        .set_menu_model(Some(&actions::more_menu_for(&menu_data)));
    header.subtitle.set_visible(false);
    if let Some(k) = &known {
        header.title.set_label(&k.title);
        header.avatar.set_label(&format::initials(&k.title));
    }
    let albums_heading = heading(&tr("Albums"));
    let albums = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .homogeneous(true)
        .min_children_per_line(2)
        .max_children_per_line(6)
        .column_spacing(12)
        .row_spacing(18)
        .build();
    albums.add_css_class("album-flow");
    albums.connect_child_activated(|_, child| {
        if let Some(d) = child.child().and_downcast::<Card>().and_then(|c| c.data()) {
            window::show_album(d.id, Some(d));
        }
    });
    let songs_heading = heading(&tr("Songs"));
    let tracks = track_box(RowStyle::TRACK, move |i| {
        actions::play(Collection::Artist { id }, i, false);
    });
    let (body, clamp) = page_body(&header);
    body.append(&albums_heading);
    body.append(&albums);
    body.append(&songs_heading);
    body.append(&tracks);
    let title = known.as_ref().map_or_else(|| tr("Artist"), |k| k.title.clone());
    let (page, _) = frame(&title, &format!("artist-{id}"), &scroller(&clamp));

    let header = Rc::new(header);
    let weak_header = Rc::downgrade(&header);
    let weak_page = page.downgrade();
    let (weak_albums, weak_tracks) = (albums.downgrade(), tracks.downgrade());
    spawn(async move {
        let result = app()
            .query(move |core| {
                let tracks = core.tracks(&TrackQuery {
                    artist_id: Some(id),
                    sort: TrackSort::Artist,
                    limit: 1_000,
                    ..TrackQuery::default()
                });
                let albums = core.artist_albums(id).ok();
                // jf-core has no artist(id): look the name up in the list.
                let name = core
                    .artists(0, 100_000)
                    .ok()
                    .and_then(|all| all.into_iter().find(|a| a.id == id))
                    .map(|a| a.name);
                let rows: Option<Vec<ItemData>> = tracks
                    .as_ref()
                    .ok()
                    .map(|t| t.iter().map(|t| convert::track_with_favorite(core, t)).collect());
                (tracks, albums, rows, name)
            })
            .await;
        let (Some(header), Some(albums), Some(list)) = (
            weak_header.upgrade(),
            weak_albums.upgrade(),
            weak_tracks.upgrade(),
        ) else {
            return;
        };
        let Some((Ok(tracks), album_list, Some(rows), name)) = result else {
            return;
        };
        header
            .meta
            .set_label(&trn("{n} song", "{n} songs", tracks.len() as u64));
        if let Some(name) = name {
            header.title.set_label(&name);
            header.avatar.set_label(&format::initials(&name));
            if let Some(page) = weak_page.upgrade() {
                page.set_title(&name);
            }
        }
        // Until artist pages land in the core, the albums come from the
        // artist's songs, in the core's order.
        let album_items: Vec<ItemData> = match album_list {
            Some(list) => list.iter().map(convert::album).collect(),
            None => {
                let mut seen = HashSet::new();
                tracks
                    .iter()
                    .filter_map(|t| {
                        let album_id = t.album_id?;
                        seen.insert(album_id).then(|| ItemData {
                            kind: Kind::Album,
                            id: album_id,
                            title: t.album.clone().unwrap_or_else(convert::unknown_album),
                            subtitle: t.year.map(|y| y.to_string()).unwrap_or_default(),
                            art: t.artwork_hash.clone().map(ArtKey::Hash),
                            album_id: Some(album_id),
                            playable: true,
                            ..ItemData::default()
                        })
                    })
                    .collect()
            }
        };
        for data in &album_items {
            let card = Card::new(150);
            card.bind(&Item::new(Some(data.clone())));
            albums.append(&card);
        }
        fill_track_box(&list, RowStyle::TRACK, &rows);
    });
    let _ = &albums_heading;
    let _ = &songs_heading;
    crate::ui::pages::keep_alive(&page, header);
    page
}
