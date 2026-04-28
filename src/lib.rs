//! GTK4 widget that renders Markdown source as native GTK widgets.
//!
//! [`MarkdownTextView`] is a [`gtk::Box`] subclass: each call to
//! [`MarkdownTextView::set_markdown`] reparses the source and rebuilds the
//! child widgets (paragraphs, headings, lists, quotes, code blocks, inline
//! code, links, emphasis).

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

    /// Replaces the rendered content with the result of parsing `text`.
    pub fn set_markdown(&self, text: &str) {
        self.imp().set_markdown(self, text);
    }

    /// Returns the current Markdown source.
    pub fn markdown(&self) -> String {
        self.imp().markdown.borrow().clone()
    }
}
