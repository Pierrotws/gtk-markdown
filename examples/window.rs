use gtk::{prelude::*, Application, ApplicationWindow, ScrolledWindow};
use gtk_markdown::MarkdownTextView;

fn sample_markdown() -> String {
    let asset = format!("{}/examples/sample.svg", env!("CARGO_MANIFEST_DIR"));
    format!(
        "# gtk-markdown\n\
         \n\
         ![gtk-markdown banner]({asset})\n\
         \n\
         A GTK4 widget that renders a *subset* of **Markdown** as native widgets.\n\
         \n\
         ## Inline\n\
         \n\
         Paragraphs collapse soft newlines into spaces, support `inline code`,\n\
         [links](https://example.invalid), and ***bold italic*** runs.\n\
         \n\
         ## Lists\n\
         \n\
         - bullets with `-`, `*`, `+`\n\
         - nested formatting like **bold** or _italic_\n\
         - arbitrary [labels](https://example.invalid)\n\
         \n\
         1. ordered\n\
         2. items\n\
         3. work too\n\
         \n\
         ## Quotes\n\
         \n\
         > Block quotes render in italic.\n\
         \n\
         ## Code blocks\n\
         \n\
         ```\n\
         fn main() {{\n\
         \x20   println!(\"hello, gtk-markdown\");\n\
         }}\n\
         ```\n"
    )
}

fn main() {
    let app = Application::builder()
        .application_id("dev.pierrotws.gtk-markdown.example")
        .build();

    app.connect_activate(|app| {
        let view = MarkdownTextView::new();
        view.set_markdown(&sample_markdown());
        view.set_margin_top(12);
        view.set_margin_bottom(12);
        view.set_margin_start(12);
        view.set_margin_end(12);

        let scroller = ScrolledWindow::builder().child(&view).build();

        ApplicationWindow::builder()
            .application(app)
            .title("gtk-markdown")
            .default_width(720)
            .default_height(560)
            .child(&scroller)
            .build()
            .present();
    });

    app.run();
}
