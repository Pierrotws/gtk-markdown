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

    /// Heading-level CSS class offset.
    ///
    /// A markdown `#` heading (level 1) maps to the GTK CSS class
    /// `title-{level + offset}`. Default offset is `0`, so `#` → `title-1`,
    /// `##` → `title-2`, etc. Use a positive offset when the widget lives
    /// inside a container that already styles its content as a high-level
    /// heading and `#` should look smaller.
    pub fn heading_level_offset(&self) -> u32 {
        self.imp().heading_level_offset.get()
    }

    /// Sets the heading-level CSS class offset and re-renders.
    pub fn set_heading_level_offset(&self, offset: u32) {
        if self.imp().heading_level_offset.get() == offset {
            return;
        }
        self.imp().heading_level_offset.set(offset);
        let text = self.markdown();
        self.imp().rebuild(self, &text);
    }
}
