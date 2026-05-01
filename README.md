# gtk-markdown

A GTK4 widget that renders a subset of Markdown as native GTK widgets.

`MarkdownTextView` is a `gtk::Box` subclass. Each call to `set_markdown` reparses
the source and rebuilds the child widgets, so headings get real `title-N` CSS
classes, links are real `<a>` markup, code blocks are framed monospace boxes,
and inline text wraps and selects like any other GTK label.

## Installation

Once published to crates.io:

```toml
[dependencies]
gtk-markdown = "0.1"
gtk = { package = "gtk4", version = "0.11" }
```

In the meantime, depend on the git source:

```toml
[dependencies]
gtk-markdown = { git = "https://github.com/Pierrotws/gtk-markdown" }
gtk = { package = "gtk4", version = "0.11" }
```

## Usage

A runnable demo lives in [`examples/window.rs`](examples/window.rs):

```sh
cargo run --example window
```

The minimal wiring is:

```rust,ignore
use gtk::{prelude::*, Application, ApplicationWindow, ScrolledWindow};
use gtk_markdown::MarkdownTextView;

fn main() {
    let app = Application::builder()
        .application_id("dev.pierrotws.gtk-markdown.example")
        .build();

    app.connect_activate(|app| {
        let view = MarkdownTextView::new();
        view.set_markdown("# Hello\n\nA *small* paragraph.");

        ApplicationWindow::builder()
            .application(app)
            .title("gtk-markdown")
            .default_width(640)
            .default_height(480)
            .child(&ScrolledWindow::builder().child(&view).build())
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
- Images (`![alt](path)`) — local file paths only; remote URLs and missing
  files fall back to an italic `[image: alt]` placeholder

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
