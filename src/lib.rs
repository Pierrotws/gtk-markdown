//! GTK4 widget that renders Markdown source as native GTK widgets.
//!
//! [`MarkdownTextView`] is a [`gtk::Box`] subclass: each call to
//! [`MarkdownTextView::set_markdown`] reparses the source and rebuilds the
//! child widgets (paragraphs, headings, lists, quotes, code blocks, inline
//! code, links, emphasis).
//!
//! `markdown` and `heading-level-offset` are exposed as `GObject`
//! properties, so they can be set from a `.ui` / `GtkBuilder`, bound via
//! [`glib::object::ObjectExt::bind_property`], and observed through
//! `notify::markdown` / `notify::heading-level-offset`.

use std::path::{Path, PathBuf};

use gtk::glib::prelude::*;
use gtk::{glib, subclass::prelude::*};

mod imp;
pub mod parser;
mod render;

pub use parser::{Emphasis, InlineSegment, MarkdownBlock};

glib::wrapper! {
    pub struct MarkdownTextView(ObjectSubclass<imp::MarkdownTextView>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl Default for MarkdownTextView {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkdownTextView {
    #[must_use]
    pub fn new() -> Self {
        glib::Object::new()
    }

    /// Returns the base path that relative image URIs are resolved against.
    #[must_use]
    pub fn base_path(&self) -> Option<PathBuf> {
        self.imp().base_path.borrow().clone()
    }

    /// Sets the base path used to resolve relative image URIs and re-renders.
    ///
    /// When `Some`, relative `src` values in `![alt](src)` are joined with
    /// `base` before checking the filesystem. When `None` (the default),
    /// relative paths are resolved against the process working directory.
    /// Absolute paths and `http(s)://` URIs are unaffected.
    pub fn set_base_path(&self, base: Option<&Path>) {
        let new = base.map(Path::to_path_buf);
        if *self.imp().base_path.borrow() == new {
            return;
        }
        *self.imp().base_path.borrow_mut() = new;
        let text = self.markdown();
        if text.is_empty() {
            return;
        }
        self.imp().rebuild(&text);
    }

    /// Connects a callback that fires when the user clicks a Markdown link.
    ///
    /// The callback receives the link's URI and returns `true` to stop the
    /// default behaviour (`gio::AppInfo::launch_default_for_uri`) or
    /// `false` to let it proceed. Useful for in-app routing (e.g.
    /// `app://...` URIs) or analytics on outbound clicks.
    ///
    /// When multiple handlers are connected, the *first* handler that
    /// returns `true` wins and later handlers are not invoked. Order
    /// follows the GLib connection order.
    pub fn connect_link_activated<F>(&self, f: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self, &str) -> bool + 'static,
    {
        self.connect_local("link-activated", false, move |args| {
            let view = args[0].get::<Self>().expect("self argument");
            let uri = args[1].get::<String>().expect("uri argument");
            Some(f(&view, &uri).to_value())
        })
    }

    pub(crate) fn emit_link_activated(&self, uri: &str) -> bool {
        self.emit_by_name::<bool>("link-activated", &[&uri.to_string()])
    }

    pub(crate) fn current_generation(&self) -> u64 {
        self.imp().generation.get()
    }
}
