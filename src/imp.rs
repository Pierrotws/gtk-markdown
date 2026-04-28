use std::cell::RefCell;

use gtk::{glib, prelude::*, subclass::prelude::*};

use crate::render;

#[derive(Default)]
pub struct MarkdownTextView {
    pub markdown: RefCell<String>,
}

#[glib::object_subclass]
impl ObjectSubclass for MarkdownTextView {
    const NAME: &'static str = "MarkdownTextView";
    type Type = super::MarkdownTextView;
    type ParentType = gtk::Box;
}

impl ObjectImpl for MarkdownTextView {
    fn constructed(&self) {
        self.parent_constructed();
        let obj = self.obj();
        obj.set_orientation(gtk::Orientation::Vertical);
    }
}

impl WidgetImpl for MarkdownTextView {}
impl BoxImpl for MarkdownTextView {}

impl MarkdownTextView {
    pub fn set_markdown(&self, obj: &super::MarkdownTextView, text: &str) {
        let container: &gtk::Box = obj.upcast_ref();
        clear_box(container);
        render::render_into(container, text);
        *self.markdown.borrow_mut() = text.to_string();
    }
}

fn clear_box(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}
