//! The queue panel: what plays next, in playback order. Rows reorder by
//! drag and drop, remove with their × button, and play when clicked.
//!
//! The queue slice carries the next 50 entries; longer queues are paged
//! with `Core::queue_window`. Entries carry only a track reference, so rows
//! are looked up on the query thread and kept by entry uid.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use adw::prelude::*;
use jf_core::{Command, QueueItem, TrackRef};

use crate::logic::queue::{move_target, up_next_index};
use crate::ui::convert;
use crate::ui::dnd;
use crate::ui::i18n::{tr, trf, trn};
use crate::ui::models::{ItemData, PagedModel};
use crate::ui::rows::{Handler, RowStyle, row_factory};
use crate::ui::state::{Change, app, spawn};
use crate::ui::widgets::glyph;

pub struct Queue {
    pub root: gtk::Box,
    model: PagedModel,
    count: gtk::Label,
    clear: gtk::Button,
    stack: gtk::Stack,
    remote: adw::StatusPage,
    /// The service whose remote session plays, if one does.
    remote_service: RefCell<Option<String>>,
    cache: Rc<RefCell<HashMap<u64, ItemData>>>,
}

impl std::fmt::Debug for Queue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Queue").finish_non_exhaustive()
    }
}

/// Rows for queue entries: cached ones at once, the rest looked up.
async fn rows_for(items: Vec<QueueItem>, cache: Rc<RefCell<HashMap<u64, ItemData>>>) -> Vec<ItemData> {
    let missing: Vec<(u64, i64)> = items
        .iter()
        .filter(|i| !cache.borrow().contains_key(&i.uid))
        .filter_map(|i| match i.track {
            TrackRef::Library(id) => Some((i.uid, id)),
            _ => None,
        })
        .collect();
    if !missing.is_empty() {
        let found = app()
            .query(move |core| {
                missing
                    .into_iter()
                    .filter_map(|(uid, id)| {
                        core.track(id)
                            .ok()
                            .flatten()
                            .map(|t| (uid, convert::queue_track(uid, &t)))
                    })
                    .collect::<Vec<_>>()
            })
            .await
            .unwrap_or_default();
        let mut c = cache.borrow_mut();
        for (uid, data) in found {
            c.insert(uid, data);
        }
    }
    let c = cache.borrow();
    items
        .iter()
        .map(|i| {
            c.get(&i.uid)
                .cloned()
                .unwrap_or_else(|| convert::queue_entry(i.uid, &i.track))
        })
        .collect()
}

impl Queue {
    pub fn new() -> Rc<Queue> {
        let cache: Rc<RefCell<HashMap<u64, ItemData>>> = Rc::new(RefCell::new(HashMap::new()));
        let loader_cache = cache.clone();
        let model = PagedModel::new(move |model, offset, limit, generation| {
            let weak = model.downgrade();
            let cache = loader_cache.clone();
            spawn(async move {
                let a = app();
                let (position, up_next) = {
                    let q = a.queue.borrow();
                    (q.position, q.up_next.clone())
                };
                let start = offset as usize;
                let end = start + limit as usize;
                let items: Vec<QueueItem> = if end <= up_next.len() {
                    up_next[start..end].to_vec()
                } else {
                    let base = up_next_index(position, start);
                    a.query(move |core| core.queue_window(base, limit as usize))
                        .await
                        .unwrap_or_default()
                };
                let rows = rows_for(items, cache).await;
                if let Some(model) = weak.upgrade() {
                    model.deliver(generation, offset, rows);
                }
            });
        });
        let on_remove: Handler = Rc::new(|data: &ItemData| {
            app().send(Command::RemoveFromQueue { uid: data.uid });
        });
        let weak_model = model.downgrade();
        let on_move: dnd::OnMove = Rc::new(move |from, target, side| {
            let Some(model) = weak_model.upgrade() else { return };
            let Some(entry) = model.data_at(from) else { return };
            if let Some(to) = move_target(from as usize, target as usize, side, model.len() as usize) {
                let position = app().queue.borrow().position;
                app().send(Command::MoveInQueue {
                    uid: entry.uid,
                    to: up_next_index(position, to),
                });
            }
        });
        let factory = row_factory(RowStyle::QUEUE, Some(on_remove));
        factory.connect_setup(move |_, obj| {
            if let Some(li) = obj.downcast_ref::<gtk::ListItem>()
                && let Some(row) = li.child()
            {
                dnd::attach(&row, li, on_move.clone());
            }
        });
        let list = gtk::ListView::builder()
            .model(&gtk::NoSelection::new(Some(model.clone())))
            .factory(&factory)
            .single_click_activate(true)
            .build();
        list.add_css_class("queue-list");
        list.update_property(&[gtk::accessible::Property::Label(&tr("Up Next"))]);
        let weak_model = model.downgrade();
        list.connect_activate(move |_, position| {
            if let Some(data) = weak_model.upgrade().and_then(|m| m.data_at(position)) {
                app().send(Command::JumpTo { uid: data.uid });
            }
        });

        let title = gtk::Label::builder()
            .label(tr("Up Next"))
            .xalign(0.0)
            .accessible_role(gtk::AccessibleRole::Heading)
            .build();
        title.add_css_class("panel-title");
        let count = gtk::Label::new(None);
        count.add_css_class("panel-count");
        count.set_xalign(0.0);
        count.set_hexpand(true);
        let clear = glyph::button("clear", 18, &tr("Clear Queue"));
        clear.add_css_class("np-icon-button");
        clear.connect_clicked(|_| confirm_clear());
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        head.add_css_class("panel-head");
        head.append(&title);
        head.append(&count);
        head.append(&clear);

        let stack = gtk::Stack::new();
        let scroll = gtk::ScrolledWindow::builder()
            .child(&list)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .build();
        stack.add_named(&scroll, Some("list"));
        let empty = adw::StatusPage::builder()
            .title(tr("Nothing Up Next"))
            .description(tr("Use Play Next or Play Last on any song, album or playlist."))
            .build();
        empty.add_css_class("compact");
        stack.add_named(&empty, Some("empty"));
        // While a remote session plays, its queue is the service's.
        let remote = adw::StatusPage::new();
        remote.add_css_class("compact");
        stack.add_named(&remote, Some("remote"));

        let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
        root.add_css_class("queue-panel");
        root.append(&head);
        root.append(&stack);
        let queue = Rc::new(Queue {
            root,
            model,
            count,
            clear,
            stack,
            remote,
            remote_service: RefCell::new(None),
            cache,
        });
        queue.refresh();
        let weak = Rc::downgrade(&queue);
        app().listen_for(&queue.root, move |change| {
            if let (Change::Queue, Some(q)) = (change, weak.upgrade()) {
                q.refresh();
            }
        });
        queue
    }

    /// While a remote session plays (`service`), the pane says that Up
    /// Next is in the service; afterwards the queue shows again.
    pub fn set_remote(&self, service: Option<&str>) {
        let service = service.map(str::to_owned);
        if *self.remote_service.borrow() == service {
            return;
        }
        *self.remote_service.borrow_mut() = service.clone();
        if let Some(name) = &service {
            self.remote
                .set_description(Some(&trf("Up Next is in {service}.", &[("service", name)])));
        }
        self.refresh();
    }

    fn refresh(&self) {
        if self.remote_service.borrow().is_some() {
            self.count.set_label("");
            self.clear.set_visible(false);
            self.stack.set_visible_child_name("remote");
            return;
        }
        self.clear.set_visible(true);
        let (len, position) = {
            let a = app();
            let q = a.queue.borrow();
            (q.len, q.position)
        };
        let after = len.saturating_sub(position.map_or(0, |p| p + 1));
        let n = u32::try_from(after).unwrap_or(u32::MAX);
        self.model.reset(n);
        self.count.set_label(&if n > 0 {
            trn("{n} song", "{n} songs", u64::from(n))
        } else {
            String::new()
        });
        self.stack
            .set_visible_child_name(if n == 0 { "empty" } else { "list" });
        // Forget rows of entries no longer queued (bounded memory).
        if self.cache.borrow().len() > 4 * (n as usize).max(256) {
            self.cache.borrow_mut().clear();
        }
    }
}

fn confirm_clear() {
    spawn(async {
        let a = app();
        let dialog = adw::AlertDialog::new(
            Some(&tr("Clear the Queue?")),
            Some(&tr("Playback stops and every song is removed from the queue.")),
        );
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("clear", &tr("Clear Queue"));
        dialog.set_response_appearance("clear", adw::ResponseAppearance::Destructive);
        dialog.set_close_response("cancel");
        let parent = a.window.borrow().upgrade();
        if dialog.choose_future(parent.as_ref()).await == "clear" {
            a.send(Command::ClearQueue);
        }
    });
}
