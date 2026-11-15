//! Favourites: every song marked with a heart.

use adw::prelude::*;
use jf_core::Collection;

use crate::ui::actions;
use crate::ui::convert;
use crate::ui::i18n::tr;
use crate::ui::models::{Item, ItemData, store};
use crate::ui::pages::library::{LIST_WIDTH, clamped};
use crate::ui::pages::{frame, show_small_title, status, titled_scroller};
use crate::ui::rows::{RowStyle, row_factory};
use crate::ui::state::{Change, app, spawn};
use crate::ui::widgets::glyph;

/// Favourites come in pages of this size until a short page ends the list
/// (the core gives no count).
const PAGE: usize = 200;

pub fn page() -> adw::NavigationPage {
    let model = store(Vec::<ItemData>::new());
    let list = gtk::ListView::builder()
        .model(&gtk::NoSelection::new(Some(model.clone())))
        .factory(&row_factory(RowStyle::TRACK, None))
        .single_click_activate(true)
        .build();
    list.add_css_class("track-list");
    list.update_property(&[gtk::accessible::Property::Label(&tr("Favourites"))]);
    list.connect_activate(|_, position| actions::play(Collection::Favorites, position as usize, false));
    let (scroll, small) = titled_scroller(&tr("Favourites"), &clamped(&list), Some(LIST_WIDTH));
    let stack = gtk::Stack::new();
    stack.add_named(&scroll, Some("list"));
    stack.add_named(
        &status(
            "starred-symbolic",
            &tr("No Favourites Yet"),
            &tr("Songs you mark with the heart appear here."),
        ),
        Some("empty"),
    );
    let error = status(
        "dialog-information-symbolic",
        &tr("Could Not Load Favourites"),
        "",
    );
    stack.add_named(&error, Some("error"));
    let (page, header) = frame(&tr("Favourites"), "favourites", &stack);
    show_small_title(&header, &small);
    let shuffle = glyph::button("shuffle", 18, &tr("Shuffle"));
    shuffle.connect_clicked(|_| actions::play(Collection::Favorites, 0, true));
    header.pack_end(&shuffle);
    let play = glyph::button("play", 18, &tr("Play"));
    play.connect_clicked(|_| actions::play(Collection::Favorites, 0, false));
    header.pack_end(&play);

    let refresh = {
        let model = model.downgrade();
        let stack = stack.downgrade();
        let error = error.downgrade();
        move || {
            let (model, stack, error) = (model.clone(), stack.clone(), error.clone());
            spawn(async move {
                let result = app()
                    .query(|core| {
                        let mut all = Vec::new();
                        loop {
                            let page = core.favorites(all.len(), PAGE).map_err(|e| e.to_string())?;
                            let short = page.len() < PAGE;
                            all.extend(page.iter().map(convert::track_row));
                            if short {
                                return Ok::<_, String>(all);
                            }
                        }
                    })
                    .await;
                let (Some(model), Some(stack), Some(error)) =
                    (model.upgrade(), stack.upgrade(), error.upgrade())
                else {
                    return;
                };
                match result {
                    Some(Ok(rows)) => {
                        let items: Vec<Item> = rows.into_iter().map(|d| Item::new(Some(d))).collect();
                        model.splice(0, model.n_items(), &items);
                        stack.set_visible_child_name(if items.is_empty() { "empty" } else { "list" });
                    }
                    Some(Err(e)) => {
                        error.set_description(Some(&e));
                        stack.set_visible_child_name("error");
                    }
                    None => {}
                }
            });
        }
    };
    refresh();
    let r = refresh.clone();
    page.connect_showing(move |_| r());
    app().listen_for(&page, move |change| {
        if matches!(change, Change::Library) {
            refresh();
        }
    });
    page
}
