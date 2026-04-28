# gtk-markdown

A GTK4 widget that renders a subset of Markdown as native GTK widgets.

`MarkdownTextView` is a `gtk::Box` subclass. Each call to `set_markdown` reparses
the source and rebuilds the child widgets, so headings get real `title-N` CSS
classes, links are real `<a>` markup, code blocks are framed monospace boxes,
and inline text wraps and selects like any other GTK label.

## Installation

```toml
[dependencies]
gtk-markdown = { git = "https://github.com/Pierrotws/gtk-markdown" }
gtk = { package = "gtk4", version = "0.11" }
```

## Usage

```rust
use gtk::{prelude::*, Application, ApplicationWindow, ScrolledWindow};
use gtk_markdown::MarkdownTextView;

const SAMPLE: &str = "\
# Hello

A *small* paragraph with `inline code` and a [link](https://example.invalid).

- one
- two
- three

```
fn main() {
    println!(\"hi\");
}
```
";

fn main() {
    let app = Application::builder()
        .application_id("dev.pierrotws.gtk-markdown.example")
        .build();

    app.connect_activate(|app| {
        let view = MarkdownTextView::new();
        view.set_markdown(SAMPLE);

        let scroller = ScrolledWindow::builder().child(&view).build();

        ApplicationWindow::builder()
            .application(app)
            .title("gtk-markdown")
            .default_width(640)
            .default_height(480)
            .child(&scroller)
            .build()
            .present();
    });

    app.run();
}
```

## Supported Markdown

Block-level:

- ATX headings (`#` through `######`)
- Paragraphs (soft newlines collapse to spaces, blank lines split paragraphs)
- Block quotes (`> ...`)
- Unordered list items (`-`, `*`, `+`)
- Ordered list items (`N.`)
- Fenced code blocks (```` ``` ````)

Inline:

- Bold (`**x**`, `__x__`)
- Italic (`*x*`, `_x_`)
- Bold+italic (`***x***`, `___x___`)
- Inline code (`` `x` ``)
- Links (`[label](uri)`)

Anything outside that subset is rendered as plain text.

## Heading-level CSS offset

Headings render as labels with the GTK CSS class `title-{level + offset}`. The
default offset is `0`, so `#` → `title-1`, `##` → `title-2`, and so on.

Use a positive offset when the widget lives inside a container that already
styles its content as a high-level heading and `#` should render smaller:

```rust
view.set_heading_level_offset(1); // `#` now uses `title-2`
```

## License

Licensed under the [GNU General Public License, version 3 or later](LICENSE).
