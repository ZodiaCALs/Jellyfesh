//! Drag-and-drop reordering for list views (queue, playlist entries).
//!
//! Each row is a drag source carrying its position and a drop target; a
//! drop above or below a row becomes one move command (`logic::queue`).

use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};

use crate::logic::queue::Drop;

/// Called with (from, target row, side) when a row is dropped.
pub type OnMove = Rc<dyn Fn(u32, u32, Drop)>;

/// Makes the row inside `item` draggable and a drop target.
pub fn attach(row: &impl IsA<gtk::Widget>, item: &gtk::ListItem, on_move: OnMove) {
    let row = row.as_ref();
    let source = gtk::DragSource::new();
    source.set_actions(gdk::DragAction::MOVE);
    let weak_item = item.downgrade();
    source.connect_prepare(move |_, _, _| {
        let item = weak_item.upgrade()?;
        Some(gdk::ContentProvider::for_value(&item.position().to_value()))
    });
    let weak_row = row.downgrade();
    source.connect_drag_begin(move |src, _| {
        if let Some(row) = weak_row.upgrade() {
            let paintable = gtk::WidgetPaintable::new(Some(&row));
            src.set_icon(Some(&paintable), 24, 24);
            row.add_css_class("dragging");
        }
    });
    let weak_row = row.downgrade();
    source.connect_drag_end(move |_, _, _| {
        if let Some(row) = weak_row.upgrade() {
            row.remove_css_class("dragging");
        }
    });
    row.add_controller(source);

    let target = gtk::DropTarget::new(glib::Type::U32, gdk::DragAction::MOVE);
    let weak_row = row.downgrade();
    target.connect_motion(move |_, _, y| {
        if let Some(row) = weak_row.upgrade() {
            let side = Drop::at(y, f64::from(row.height()));
            let classes = classes_with(&row, side);
            let refs: Vec<&str> = classes.iter().map(String::as_str).collect();
            row.set_css_classes(&refs);
        }
        gdk::DragAction::MOVE
    });
    let weak_row = row.downgrade();
    target.connect_leave(move |_| {
        if let Some(row) = weak_row.upgrade() {
            row.remove_css_class("drop-above");
            row.remove_css_class("drop-below");
        }
    });
    let weak_item = item.downgrade();
    let weak_row = row.downgrade();
    target.connect_drop(move |_, value, _, y| {
        let (Some(item), Some(row)) = (weak_item.upgrade(), weak_row.upgrade()) else {
            return false;
        };
        row.remove_css_class("drop-above");
        row.remove_css_class("drop-below");
        let Ok(from) = value.get::<u32>() else {
            return false;
        };
        let side = Drop::at(y, f64::from(row.height()));
        on_move(from, item.position(), side);
        true
    });
    row.add_controller(target);
}

fn classes_with(row: &gtk::Widget, side: Drop) -> Vec<String> {
    let mut classes: Vec<String> = row
        .css_classes()
        .iter()
        .map(|c| c.to_string())
        .filter(|c| c != "drop-above" && c != "drop-below")
        .collect();
    classes.push(match side {
        Drop::Before => "drop-above".into(),
        Drop::After => "drop-below".into(),
    });
    classes
}
