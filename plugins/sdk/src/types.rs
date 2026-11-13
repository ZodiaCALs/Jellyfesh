//! Constructors and builder methods for the WIT records plugins return most.

use crate::{
    FieldKind, FieldSpec, FlowStep, Header, ImageRef, Item, ItemKind, ItemPage, LyricFormat, LyricPayload,
    Step, StreamInfo, Unplayable, UnplayableReason,
};

impl Item {
    /// An item with every optional field empty, playable and not explicit.
    pub fn new(kind: ItemKind, id: impl Into<String>, title: impl Into<String>) -> Item {
        Item {
            id: id.into(),
            kind,
            title: title.into(),
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

    /// Sets the display artist.
    pub fn artist(mut self, artist: impl Into<String>) -> Item {
        self.artist = Some(artist.into());
        self
    }

    /// Sets the album title.
    pub fn album(mut self, album: impl Into<String>) -> Item {
        self.album = Some(album.into());
        self
    }

    /// Sets the duration.
    pub fn duration_ms(mut self, duration_ms: u32) -> Item {
        self.duration_ms = Some(duration_ms);
        self
    }

    /// Sets the ISRC.
    pub fn isrc(mut self, isrc: impl Into<String>) -> Item {
        self.isrc = Some(isrc.into());
        self
    }

    /// Sets the artwork.
    pub fn artwork(mut self, url: impl Into<String>) -> Item {
        self.artwork = Some(ImageRef::new(url));
        self
    }

    /// Marks the item as not playable for this account, with the reason. The
    /// host shows it and never works around it.
    pub fn unplayable(mut self, reason: UnplayableReason, message: Option<String>) -> Item {
        self.playable = false;
        self.unplayable = Some(Unplayable { reason, message });
        self
    }
}

impl ImageRef {
    /// An image without extra headers or known size.
    pub fn new(url: impl Into<String>) -> ImageRef {
        ImageRef {
            url: url.into(),
            headers: Vec::new(),
            width: None,
            height: None,
        }
    }
}

impl ItemPage {
    /// A last page.
    pub fn new(items: Vec<Item>) -> ItemPage {
        ItemPage {
            items,
            next: None,
            total: None,
        }
    }

    /// A page followed by another one at `cursor`.
    pub fn with_next(items: Vec<Item>, cursor: impl Into<String>) -> ItemPage {
        ItemPage {
            items,
            next: Some(cursor.into()),
            total: None,
        }
    }
}

impl FlowStep {
    /// A step without state, polled after the default interval.
    pub fn new(step: Step) -> FlowStep {
        FlowStep {
            step,
            state: String::new(),
            poll_after_ms: None,
        }
    }

    /// Keeps `state` for the next call (`flow.state`).
    pub fn state(mut self, state: impl Into<String>) -> FlowStep {
        self.state = state.into();
        self
    }

    /// Asks the host to poll after `ms` milliseconds.
    pub fn poll_after_ms(mut self, ms: u32) -> FlowStep {
        self.poll_after_ms = Some(ms);
        self
    }

    /// Logged in as `account`.
    pub fn done(account: impl Into<String>) -> FlowStep {
        FlowStep::new(Step::Done(account.into()))
    }

    /// The login failed.
    pub fn failed(message: impl Into<String>) -> FlowStep {
        FlowStep::new(Step::Failed(message.into()))
    }
}

impl FieldSpec {
    /// A required plain-text field.
    pub fn text(name: impl Into<String>, label: impl Into<String>) -> FieldSpec {
        FieldSpec {
            name: name.into(),
            label: label.into(),
            kind: FieldKind::Text,
            secret: false,
            required: true,
            placeholder: None,
            help: None,
        }
    }

    /// A required secret field (password, API key).
    pub fn secret(name: impl Into<String>, label: impl Into<String>) -> FieldSpec {
        FieldSpec {
            secret: true,
            ..FieldSpec::text(name, label)
        }
    }
}

impl StreamInfo {
    /// A stream URL without headers, expiry or hints; not cacheable.
    pub fn new(url: impl Into<String>) -> StreamInfo {
        StreamInfo {
            url: url.into(),
            headers: Vec::new(),
            expires_at_ms: None,
            codec: None,
            bitrate_kbps: None,
            cacheable: false,
        }
    }
}

impl LyricPayload {
    /// Lyrics in `format`.
    pub fn new(format: LyricFormat, text: impl Into<String>) -> LyricPayload {
        LyricPayload {
            format,
            text: text.into(),
            language: None,
            attribution: None,
        }
    }
}

impl Header {
    /// A header.
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Header {
        Header {
            name: name.into(),
            value: value.into(),
        }
    }
}
