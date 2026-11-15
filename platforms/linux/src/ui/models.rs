//! List items and the lazily paged list model.
//!
//! One item type serves every list (albums, artists, songs, playlists, queue
//! entries, search and browse results): it carries what a row shows and what
//! a click plays. A [`PagedModel`] knows only its length; when a view binds
//! an item whose page is missing, the model asks its loader (a query on the
//! query thread) and fills the placeholder items when the page arrives, so
//! bound rows update in place.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::OnceLock;

use gtk::gio;
use gtk::glib::{self, subclass::Signal};
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use jf_core::{SourceItem, TrackRef};

use crate::logic::paging::{PAGE_SIZE, Pages, page_range};
use crate::ui::thumbs::ArtKey;

/// What an item stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Kind {
    #[default]
    Track,
    Album,
    Artist,
    Playlist,
    /// A queue entry (`uid` set).
    QueueEntry,
    /// A playlist entry (`id` is the entry id).
    PlaylistEntry,
    /// An item from a source plugin (search or browse).
    Source,
}

/// What a row shows and acts on.
#[derive(Debug, Clone, Default)]
pub struct ItemData {
    pub kind: Kind,
    /// Library id of the album, artist, track or playlist; entry id for
    /// playlist entries.
    pub id: i64,
    pub title: String,
    pub subtitle: String,
    /// Right-hand detail: a duration, a year, a count.
    pub detail: String,
    /// Track number or list position.
    pub number: Option<u32>,
    pub art: Option<ArtKey>,
    pub track: Option<TrackRef>,
    pub album_id: Option<i64>,
    pub artist_id: Option<i64>,
    /// A library song's artist as tagged, for Go to Artist (the core gives
    /// songs an artist name, not an id).
    pub artist: Option<String>,
    pub uid: u64,
    pub favorite: bool,
    /// False for missing files and items a source will not play here.
    pub playable: bool,
    pub source: Option<Box<SourceItem>>,
}

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct Item {
        pub data: RefCell<Option<ItemData>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Item {
        const NAME: &'static str = "JfItem";
        type Type = super::Item;
    }

    impl ObjectImpl for Item {
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| vec![Signal::builder("changed").build()])
        }
    }
}

glib::wrapper! {
    /// A list item; empty (a placeholder) until its page is loaded.
    pub struct Item(ObjectSubclass<imp::Item>);
}

impl Item {
    pub fn new(data: Option<ItemData>) -> Item {
        let item: Item = glib::Object::new();
        *item.imp().data.borrow_mut() = data;
        item
    }

    pub fn data(&self) -> Option<ItemData> {
        self.imp().data.borrow().clone()
    }

    /// Reads the data without cloning it.
    pub fn with<R>(&self, f: impl FnOnce(Option<&ItemData>) -> R) -> R {
        f(self.imp().data.borrow().as_ref())
    }

    pub fn set_data(&self, data: Option<ItemData>) {
        *self.imp().data.borrow_mut() = data;
        self.emit_by_name::<()>("changed", &[]);
    }

    /// Calls `f` whenever the data changes.
    pub fn connect_changed<F: Fn(&Item) + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("changed", false, move |values| {
            if let Ok(item) = values[0].get::<Item>() {
                f(&item);
            }
            None
        })
    }
}

/// Loads `limit` items from `offset`, then calls `PagedModel::deliver`.
pub type Loader = Rc<dyn Fn(&PagedModel, u32, u32, u64)>;

mod paged {
    use super::*;

    #[derive(Default)]
    pub struct PagedModel {
        pub items: RefCell<Vec<Option<Item>>>,
        pub pages: RefCell<Pages>,
        pub loader: RefCell<Option<Loader>>,
        pub page_size: Cell<u32>,
    }

    impl std::fmt::Debug for PagedModel {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("PagedModel")
                .field("len", &self.items.borrow().len())
                .finish_non_exhaustive()
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PagedModel {
        const NAME: &'static str = "JfPagedModel";
        type Type = super::PagedModel;
        type Interfaces = (gio::ListModel,);
    }

    impl ObjectImpl for PagedModel {}

    impl ListModelImpl for PagedModel {
        fn item_type(&self) -> glib::Type {
            Item::static_type()
        }

        fn n_items(&self) -> u32 {
            u32::try_from(self.items.borrow().len()).unwrap_or(u32::MAX)
        }

        fn item(&self, position: u32) -> Option<glib::Object> {
            let index = position as usize;
            let (item, total) = {
                let mut items = self.items.borrow_mut();
                let total = items.len();
                let slot = items.get_mut(index)?;
                (slot.get_or_insert_with(|| Item::new(None)).clone(), total)
            };
            let page_size = self.page_size.get().max(1);
            let wanted =
                self.pages
                    .borrow_mut()
                    .wanted(position, page_size, u32::try_from(total).unwrap_or(u32::MAX));
            if !wanted.is_empty() {
                let obj = self.obj().clone();
                // Load outside the view's call into the model.
                glib::idle_add_local_once(move || {
                    for page in wanted {
                        obj.load_page(page);
                    }
                });
            }
            Some(item.upcast())
        }
    }
}

glib::wrapper! {
    /// A list model that loads its items page by page.
    pub struct PagedModel(ObjectSubclass<paged::PagedModel>) @implements gio::ListModel;
}

impl PagedModel {
    pub fn new(loader: impl Fn(&PagedModel, u32, u32, u64) + 'static) -> PagedModel {
        let m: PagedModel = glib::Object::new();
        m.imp().page_size.set(PAGE_SIZE);
        *m.imp().loader.borrow_mut() = Some(Rc::new(loader));
        m
    }

    pub fn len(&self) -> u32 {
        self.n_items()
    }

    pub fn is_empty(&self) -> bool {
        self.n_items() == 0
    }

    /// Sets the item count and forgets every page (the data changed).
    pub fn reset(&self, len: u32) {
        let old = self.n_items();
        self.imp().pages.borrow_mut().reset();
        *self.imp().items.borrow_mut() = vec![None; len as usize];
        self.items_changed(0, old, len);
    }

    /// The data changed. With the same length the loaded pages are fetched
    /// again and filled in place, so rows do not flicker (a favourite, a play
    /// count); with another length the model starts over.
    pub fn reload(&self, len: u32) {
        if len != self.n_items() {
            self.reset(len);
            return;
        }
        let page_size = self.imp().page_size.get().max(1);
        let loaded: std::collections::BTreeSet<u32> = self
            .imp()
            .items
            .borrow()
            .iter()
            .enumerate()
            .filter(|(_, item)| item.is_some())
            .map(|(i, _)| i as u32 / page_size)
            .collect();
        self.imp().pages.borrow_mut().reset();
        for page in loaded {
            self.imp().pages.borrow_mut().request(page);
            self.load_page(page);
        }
    }

    fn load_page(&self, page: u32) {
        let total = self.n_items();
        let range = page_range(page, self.imp().page_size.get(), total);
        if range.is_empty() {
            return;
        }
        let generation = self.imp().pages.borrow().generation();
        let loader = self.imp().loader.borrow().clone();
        if let Some(loader) = loader {
            loader(self, range.start, range.end - range.start, generation);
        }
    }

    /// Fills items from `offset` with a loaded page; ignored when the model
    /// was reset since the request.
    pub fn deliver(&self, generation: u64, offset: u32, rows: Vec<ItemData>) {
        if !self.imp().pages.borrow().accepts(generation) {
            return;
        }
        let mut filled = Vec::with_capacity(rows.len());
        {
            let mut items = self.imp().items.borrow_mut();
            for (k, data) in rows.into_iter().enumerate() {
                let index = offset as usize + k;
                let Some(slot) = items.get_mut(index) else { break };
                let item = slot.get_or_insert_with(|| Item::new(None)).clone();
                filled.push((item, data));
            }
        }
        for (item, data) in filled {
            item.set_data(Some(data));
        }
    }

    /// A page could not be loaded; it is asked for again when shown.
    pub fn failed(&self, generation: u64, offset: u32) {
        let page = offset / self.imp().page_size.get().max(1);
        self.imp().pages.borrow_mut().failed(page, generation);
    }

    /// The loaded data at `index`, if any.
    pub fn data_at(&self, index: u32) -> Option<ItemData> {
        self.imp()
            .items
            .borrow()
            .get(index as usize)
            .and_then(|i| i.as_ref())
            .and_then(Item::data)
    }
}

/// A fixed list of items.
pub fn store(rows: impl IntoIterator<Item = ItemData>) -> gio::ListStore {
    let store = gio::ListStore::new::<Item>();
    let items: Vec<Item> = rows.into_iter().map(|d| Item::new(Some(d))).collect();
    store.extend_from_slice(&items);
    store
}
