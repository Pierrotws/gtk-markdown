//! GTK4 widget that renders Markdown source as native GTK widgets.
//!
//! [`MarkdownTextView`] is a [`gtk::Box`] subclass: each call to
//! [`MarkdownTextView::set_markdown`] reparses the source and rebuilds the
//! child widgets (paragraphs, headings, lists, quotes, code blocks, inline
//! code, links, emphasis).
//!
//! `markdown` and `heading-level-offset` are exposed as GObject properties,
//! so they can be set from a `.ui` / GtkBuilder, bound via
//! [`glib::object::ObjectExt::bind_property`], and observed through
//! `notify::markdown` / `notify::heading-level-offset`.

use std::path::{Path, PathBuf};

use gtk::{glib, subclass::prelude::*};

mod imp;
mod parser;
mod render;

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
    pub fn new() -> Self {
        glib::Object::new()
    }

    /// Returns the base path that relative image URIs are resolved against.
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
        self.imp().rebuild(&text);
    }
}
