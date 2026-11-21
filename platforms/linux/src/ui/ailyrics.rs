//! AI Lyrics' steps (docs/ui.md "AI Lyrics, Customize and Pro"; ADR-0019):
//! making lyrics (the Pro sheet while Pro is inactive, and "Send This Song
//! to {origin}?" before the first online job), Delete Made Lyrics (asks
//! first) and Open AI Lyrics Settings. What is needed first is the core's
//! (`logic::ailyrics` reads it).

use adw::prelude::*;
use jf_core::{Command, SongRef};

use crate::logic::ailyrics::{self, Step};
use crate::ui::i18n::{tr, trf};
use crate::ui::state::{app, spawn};

/// Makes lyrics for `song`, asking first what the core says it needs.
pub fn make(song: SongRef) {
    let step = ailyrics::make(&app().ai_lyrics.borrow(), song);
    match step {
        Step::Send(command) => {
            app().send(command);
        }
        Step::ProSheet => crate::ui::pro::present_sheet(),
        Step::AskConsent { origin, allow } => spawn(async move {
            let a = app();
            let dialog = adw::AlertDialog::new(
                Some(&trf("Send This Song to {origin}?", &[("origin", &origin)])),
                Some(&trf(
                    "AI Lyrics will send the song’s audio to {origin} to make its lyrics. The provider’s terms and privacy policy apply, and it may charge your account.",
                    &[("origin", &origin)],
                )),
            );
            dialog.add_response("cancel", &tr("Cancel"));
            dialog.add_response("allow", &tr("Allow"));
            dialog.set_response_appearance("allow", adw::ResponseAppearance::Suggested);
            dialog.set_default_response(Some("allow"));
            dialog.set_close_response("cancel");
            let parent = a.window.borrow().upgrade();
            if dialog.choose_future(parent.as_ref()).await == "allow" {
                for command in allow {
                    a.send(command);
                }
            }
        }),
    }
}

/// Delete Made Lyrics for the song playing, after asking (docs/ui.md,
/// "Steps that ask first").
pub fn delete_made() {
    spawn(async {
        let a = app();
        let dialog = adw::AlertDialog::new(
            Some(&tr("Delete These Lyrics?")),
            Some(&tr("You can make them again.")),
        );
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("delete", &tr("Delete"));
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        dialog.set_close_response("cancel");
        let parent = a.window.borrow().upgrade();
        if dialog.choose_future(parent.as_ref()).await == "delete" {
            a.send(Command::DeleteMadeLyrics {
                song: SongRef::Current,
            });
        }
    });
}

/// Settings › Extensions with AI Lyrics open (its Sign In).
pub fn open_settings() {
    crate::ui::extensions::reveal("make_lyrics");
    crate::ui::window::show_settings(Some("extensions"));
}
