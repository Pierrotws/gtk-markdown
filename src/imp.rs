use std::cell::{Cell, RefCell};
use std::path::PathBuf;

use gtk::{glib, prelude::*, subclass::prelude::*};

use crate::render;

#[derive(Default, glib::Properties)]
#[properties(wrapper_type = super::MarkdownTextView)]
pub struct MarkdownTextView {
    #[property(name = "markdown", get = Self::get_markdown, set = Self::set_markdown_prop)]
    pub markdown: RefCell<String>,

    #[property(name = "heading-level-offset", get, set = Self::set_offset_prop)]
    pub heading_level_offset: Cell<u32>,

    pub base_path: RefCell<Option<PathBuf>>,
}

#[glib::object_subclass]
impl ObjectSubclass for MarkdownTextView {
    const NAME: &'static str = "MarkdownTextView";
    type Type = super::MarkdownTextView;
    type ParentType = gtk::Box;
}

#[glib::derived_properties]
impl ObjectImpl for MarkdownTextView {
    fn constructed(&self) {
        self.parent_constructed();
        self.obj().set_orientation(gtk::Orientation::Vertical);
    }
}

impl WidgetImpl for MarkdownTextView {}
impl BoxImpl for MarkdownTextView {}

impl MarkdownTextView {
    fn get_markdown(&self) -> String {
        self.markdown.borrow().clone()
    }

    fn set_markdown_prop(&self, text: String) {
        if *self.markdown.borrow() == text {
            return;
        }
        self.rebuild(&text);
    }

    fn set_offset_prop(&self, offset: u32) {
        if self.heading_level_offset.get() == offset {
            return;
        }
        self.heading_level_offset.set(offset);
        if self.markdown.borrow().is_empty() {
            return;
        }
        let text = self.markdown.borrow().clone();
        self.rebuild(&text);
    }

    pub fn rebuild(&self, text: &str) {
        let obj = self.obj();
        let container: &gtk::Box = obj.upcast_ref();
        clear_box(container);
        let base = self.base_path.borrow();
        render::render_into(
            container,
            text,
            self.heading_level_offset.get(),
            base.as_deref(),
        );
        drop(base);
        *self.markdown.borrow_mut() = text.to_string();
    }
}

fn clear_box(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}
