//! Downloads (docs/ui.md, "Downloads"; ADR-0025): what is downloading,
//! then the downloaded albums and songs, with the space they use.

use adw::prelude::*;

use crate::ui::i18n::tr;
use crate::ui::pages::{follow_title, frame, large_title, scroller, small_title, status};

pub fn page() -> adw::NavigationPage {
    let title = large_title(&tr("Downloads"));
    let empty = status(
        "folder-download-symbolic",
        &tr("No Downloads Yet"),
        &tr("Download albums and playlists from your servers to play them offline."),
    );
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.add_css_class("page-column");
    column.append(&title);
    column.append(&empty);
    let scroll = scroller(&column);
    let (page, header) = frame(&tr("Downloads"), "downloads", &scroll);
    let small = small_title(&header, &tr("Downloads"));
    follow_title(&scroll, &title, &small);
    page
}
