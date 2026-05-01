//! End-to-end coverage of the `link-activated` signal: emit semantics and
//! the activate-link → user-handler trampoline wired in `combined_label`.
//!
//! GTK pins itself to one thread, so all assertions live in a single test.
//! Needs a display (X11, Wayland, or Xvfb).

use gtk::prelude::*;
use gtk_markdown::MarkdownTextView;
use std::cell::Cell;
use std::rc::Rc;

fn first_label(view: &MarkdownTextView) -> gtk::Label {
    let container: &gtk::Box = view.upcast_ref();
    let flow = container.first_child().unwrap().downcast::<gtk::FlowBox>().unwrap();
    let flow_child = flow.first_child().unwrap();
    flow_child.first_child().unwrap().downcast::<gtk::Label>().unwrap()
}

#[test]
#[ignore = "needs a display; opt in via cargo test -- --ignored"]
fn link_activated_signal() {
    gtk::init().expect("gtk::init");

    // Accumulator: no handler -> emit returns false (so the default URI
    // launcher gets to run).
    let view = MarkdownTextView::new();
    let result: bool =
        view.emit_by_name("link-activated", &[&"https://example.invalid".to_string()]);
    assert!(!result);

    // Accumulator: first-true-wins. Single false handler keeps it false.
    let view = MarkdownTextView::new();
    let seen = Rc::new(Cell::new(None::<String>));
    let s = seen.clone();
    view.connect_link_activated(move |_, uri| {
        s.set(Some(uri.to_string()));
        false
    });
    let result: bool =
        view.emit_by_name("link-activated", &[&"https://example.invalid".to_string()]);
    assert_eq!(seen.take(), Some("https://example.invalid".to_string()));
    assert!(!result);

    // Single true handler short-circuits to true.
    let view = MarkdownTextView::new();
    view.connect_link_activated(|_, _| true);
    let result: bool =
        view.emit_by_name("link-activated", &[&"https://example.invalid".to_string()]);
    assert!(result);

    // End-to-end: a paragraph with a link produces a Label whose markup
    // contains <a href="..."> and whose activate-link routes through the
    // user's connect_link_activated.
    let view = MarkdownTextView::new();
    let observed = Rc::new(Cell::new(None::<String>));
    let o = observed.clone();
    view.connect_link_activated(move |_, uri| {
        o.set(Some(uri.to_string()));
        false
    });
    view.set_markdown("a [click me](https://example.invalid) link".to_string());

    let label = first_label(&view);
    assert!(
        label.label().contains("<a href=\"https://example.invalid\">"),
        "label markup should contain an <a href>"
    );

    let stop: bool = label.emit_by_name("activate-link", &[&"https://example.invalid"]);
    assert!(!stop, "user handler returned false; emit must propagate as false");
    assert_eq!(observed.take(), Some("https://example.invalid".to_string()));
}
