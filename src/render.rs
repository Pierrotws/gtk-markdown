//! Block / inline tokens → GTK widgets.

use std::borrow::Cow;
use std::path::Path;

use gtk::{glib, prelude::*};

use crate::parser::{
    markdown_blocks, parse_inline_segments, Emphasis, InlineSegment, InlineStyle, MarkdownBlock,
};
use crate::MarkdownTextView;

pub(crate) fn render_into(
    container: &gtk::Box,
    view: &MarkdownTextView,
    value: &str,
    heading_level_offset: u32,
    base_path: Option<&Path>,
) {
    for block in markdown_blocks(value) {
        match block {
            MarkdownBlock::Paragraph(text) => {
                container.append(&inline_flow(view, &text, InlineStyle::Normal, None, base_path));
            }
            MarkdownBlock::Heading { level, text } => {
                let css_level = level.saturating_add(heading_level_offset as usize);
                container.append(&inline_flow(
                    view,
                    &text,
                    InlineStyle::Heading(css_level),
                    None,
                    base_path,
                ));
            }
            MarkdownBlock::Quote(text) => {
                container.append(&inline_flow(view, &text, InlineStyle::Quote, None, base_path));
            }
            MarkdownBlock::List { ordered, start, items } => {
                container.append(&list_box(view, ordered, start, &items, base_path));
            }
            MarkdownBlock::Code(code) => container.append(&code_block_frame(&code)),
            MarkdownBlock::HorizontalRule => {
                let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
                // The outer Box has no inter-child spacing, so without
                // margins the Separator sits flush against neighbouring
                // text and reads as a hairline jammed into prose.
                separator.set_margin_top(HORIZONTAL_RULE_MARGIN_PX);
                separator.set_margin_bottom(HORIZONTAL_RULE_MARGIN_PX);
                container.append(&separator);
            }
        }
    }
}

fn list_box(
    view: &MarkdownTextView,
    ordered: bool,
    start: u32,
    items: &[String],
    base_path: Option<&Path>,
) -> gtk::Box {
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    for (offset, item) in items.iter().enumerate() {
        let marker = if ordered {
            format!("{}.", start + offset as u32)
        } else {
            "•".to_string()
        };
        outer.append(&inline_flow(view, item, InlineStyle::Normal, Some(&marker), base_path));
    }
    outer
}

fn inline_flow(
    view: &MarkdownTextView,
    text: &str,
    style: InlineStyle,
    marker: Option<&str>,
    base_path: Option<&Path>,
) -> gtk::FlowBox {
    let flow = gtk::FlowBox::new();
    flow.set_selection_mode(gtk::SelectionMode::None);
    flow.set_homogeneous(false);
    flow.set_column_spacing(4);
    flow.set_row_spacing(2);
    flow.set_max_children_per_line(1000);

    if let Some(marker) = marker {
        flow.insert(&text_label(marker, Emphasis::Normal, style), -1);
    }

    render_inline_segments(
        view,
        &flow,
        parse_inline_segments(text),
        Emphasis::Normal,
        style,
        base_path,
    );

    flow
}

fn render_inline_segments(
    view: &MarkdownTextView,
    flow: &gtk::FlowBox,
    segments: Vec<InlineSegment<'_>>,
    base_emphasis: Emphasis,
    style: InlineStyle,
    base_path: Option<&Path>,
) {
    let mut buffer = String::new();
    let mut has_link = false;
    accumulate_inline_segments(
        view,
        flow,
        &mut buffer,
        &mut has_link,
        segments,
        base_emphasis,
        style,
        base_path,
    );
    flush_text_buffer(view, flow, &mut buffer, &mut has_link, style);
}

#[allow(clippy::too_many_arguments)]
fn accumulate_inline_segments(
    view: &MarkdownTextView,
    flow: &gtk::FlowBox,
    buffer: &mut String,
    has_link: &mut bool,
    segments: Vec<InlineSegment<'_>>,
    base_emphasis: Emphasis,
    style: InlineStyle,
    base_path: Option<&Path>,
) {
    for segment in segments {
        match segment {
            InlineSegment::Text(text) => {
                let escaped = escape_markup(text);
                buffer.push_str(&apply_emphasis_markup(&escaped, base_emphasis));
            }
            InlineSegment::Styled { children, emphasis } => {
                let composed = combine_emphasis(base_emphasis, emphasis);
                accumulate_inline_segments(
                    view, flow, buffer, has_link, children, composed, style, base_path,
                );
            }
            InlineSegment::Link { label, uri } => {
                *has_link = true;
                let link = format!(
                    "<a href=\"{}\">{}</a>",
                    escape_markup(uri),
                    escape_markup(label)
                );
                buffer.push_str(&apply_emphasis_markup(&link, base_emphasis));
            }
            InlineSegment::Code(text) => {
                // Pure-Pango inline code: stays in the paragraph buffer so
                // the surrounding text wraps around it instead of being
                // interrupted by a separate FlowBox child.
                buffer.push_str(&code_span_markup(text));
            }
            InlineSegment::Image { alt, src } => {
                flush_text_buffer(view, flow, buffer, has_link, style);
                match picture_from_src(view, src, base_path) {
                    Some(picture) => flow.insert(&picture, -1),
                    None => flow.insert(&image_fallback_label(alt), -1),
                }
            }
        }
    }
}

fn flush_text_buffer(
    view: &MarkdownTextView,
    flow: &gtk::FlowBox,
    buffer: &mut String,
    has_link: &mut bool,
    style: InlineStyle,
) {
    let trimmed = buffer.trim();
    if !trimmed.is_empty() {
        flow.insert(&combined_label(view, trimmed, *has_link, style), -1);
    }
    buffer.clear();
    *has_link = false;
}

fn combined_label(
    view: &MarkdownTextView,
    markup: &str,
    has_link: bool,
    style: InlineStyle,
) -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_wrap(true);
    label.set_xalign(0.0);
    label.set_selectable(true);
    if let InlineStyle::Heading(level) = style {
        label.add_css_class(&heading_css_class(level));
    }
    label.set_use_markup(true);
    label.set_markup(&style_markup(markup, style));
    if has_link {
        // WeakRef breaks the View → FlowBox → Label → closure → View cycle
        // that would otherwise pin the View (and every child widget) until
        // the next set_markdown clears the children.
        let view = view.downgrade();
        label.connect_activate_link(move |_label, uri| {
            let Some(view) = view.upgrade() else {
                return glib::Propagation::Proceed;
            };
            let stop = view.emit_link_activated(uri);
            if stop {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
    }
    label
}

fn combine_emphasis(outer: Emphasis, inner: Emphasis) -> Emphasis {
    use Emphasis::{Bold, BoldItalic, Italic, Normal};
    match (outer, inner) {
        (Normal, x) | (x, Normal) => x,
        (BoldItalic, _) | (_, BoldItalic) => BoldItalic,
        (Bold, Italic) | (Italic, Bold) => BoldItalic,
        (Bold, Bold) => Bold,
        (Italic, Italic) => Italic,
    }
}

const PICTURE_CSS_CLASS: &str = "gtk-markdown-picture";
const MAX_PICTURE_HEIGHT_PX: u32 = 480;
const MIN_PICTURE_HEIGHT_PX: u32 = 64;
const MIN_PICTURE_WIDTH_PX: u32 = 64;
const HORIZONTAL_RULE_MARGIN_PX: i32 = 6;

fn picture_from_src(
    view: &MarkdownTextView,
    src: &str,
    base_path: Option<&Path>,
) -> Option<gtk::Picture> {
    if src.starts_with("http://") || src.starts_with("https://") {
        return None;
    }
    let raw = Path::new(src);
    let resolved: std::path::PathBuf = match base_path {
        Some(base) if !raw.is_absolute() => base.join(raw),
        _ => raw.to_path_buf(),
    };
    if !resolved.is_file() {
        return None;
    }
    install_picture_css_provider();
    let picture = gtk::Picture::new();
    picture.set_can_shrink(true);
    #[allow(deprecated)]
    picture.set_keep_aspect_ratio(true);
    picture.add_css_class(PICTURE_CSS_CLASS);
    spawn_paintable_loader(view, &picture, resolved);
    Some(picture)
}

fn spawn_paintable_loader(
    view: &MarkdownTextView,
    picture: &gtk::Picture,
    path: std::path::PathBuf,
) {
    use gtk::{gdk_pixbuf, gio};
    let generation = view.current_generation();
    let view = view.downgrade();
    let picture = picture.clone();
    glib::spawn_future_local(async move {
        let file = gio::File::for_path(&path);
        let Ok(stream) = file.read_future(glib::Priority::default()).await else {
            return;
        };
        let Ok(pixbuf) = gdk_pixbuf::Pixbuf::from_stream_future(&stream).await else {
            return;
        };
        // Skip if the View was dropped or a newer rebuild started while
        // we were decoding — the Picture is no longer in the tree.
        let Some(view) = view.upgrade() else { return };
        if view.current_generation() != generation {
            return;
        }
        #[allow(deprecated)]
        let texture = gtk::gdk::Texture::for_pixbuf(&pixbuf);
        picture.set_paintable(Some(&texture));
    });
}

fn install_picture_css_provider() {
    use std::sync::OnceLock;
    static INSTALLED: OnceLock<()> = OnceLock::new();
    INSTALLED.get_or_init(|| {
        let provider = gtk::CssProvider::new();
        #[allow(deprecated)]
        provider.load_from_data(&format!(
            ".{PICTURE_CSS_CLASS} {{ \
             min-width: {MIN_PICTURE_WIDTH_PX}px; \
             min-height: {MIN_PICTURE_HEIGHT_PX}px; \
             max-height: {MAX_PICTURE_HEIGHT_PX}px; \
             }}"
        ));
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });
}

fn image_fallback_label(alt: &str) -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_xalign(0.0);
    label.set_selectable(true);
    label.set_use_markup(true);
    label.set_markup(&format!("<i>[image: {}]</i>", escape_markup(alt)));
    label
}

fn text_label(text: &str, emphasis: Emphasis, style: InlineStyle) -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_wrap(true);
    label.set_xalign(0.0);
    label.set_selectable(true);
    if let InlineStyle::Heading(level) = style {
        label.add_css_class(&heading_css_class(level));
    }
    label.set_use_markup(true);
    label.set_markup(&styled_text_markup(text, emphasis, style));
    label
}

fn styled_text_markup(text: &str, emphasis: Emphasis, style: InlineStyle) -> String {
    let escaped = escape_markup(text);
    let with_emphasis = apply_emphasis_markup(&escaped, emphasis);

    style_markup(&with_emphasis, style).into_owned()
}

fn style_markup<'a>(markup: &'a str, style: InlineStyle) -> Cow<'a, str> {
    match style {
        // Heading boldness/size come from the `title-N` CSS class on the
        // Label, so we don't need an extra `<b>` wrap here.
        InlineStyle::Normal | InlineStyle::Heading(_) => Cow::Borrowed(markup),
        InlineStyle::Quote => Cow::Owned(format!("<span style=\"italic\">{markup}</span>")),
    }
}

fn apply_emphasis_markup(escaped: &str, emphasis: Emphasis) -> String {
    match emphasis {
        Emphasis::Normal => escaped.to_string(),
        Emphasis::Italic => format!("<i>{escaped}</i>"),
        Emphasis::Bold => format!("<b>{escaped}</b>"),
        Emphasis::BoldItalic => format!("<b><i>{escaped}</i></b>"),
    }
}

// Inline code: pure-Pango pill (gray background, white foreground) so the
// span flows with the surrounding paragraph text instead of breaking it
// into a separate FlowBox child.
const INLINE_CODE_SPAN_ATTRS: &str =
    "font_family=\"monospace\" background=\"#888888\" foreground=\"#ffffff\"";

fn code_span_markup(text: &str) -> String {
    format!("<span {INLINE_CODE_SPAN_ATTRS}>{}</span>", escape_markup(text))
}

fn code_block_frame(text: &str) -> gtk::Frame {
    let label = gtk::Label::new(None);
    label.set_selectable(true);
    label.set_use_markup(true);
    label.set_xalign(0.0);
    label.set_markup(&format!(
        "<span font_family=\"monospace\">{}</span>",
        escape_markup(text)
    ));
    label.set_margin_top(8);
    label.set_margin_bottom(8);
    label.set_margin_start(8);
    label.set_margin_end(8);

    let frame = gtk::Frame::new(None);
    frame.set_hexpand(true);
    frame.set_child(Some(&label));
    frame
}

fn heading_css_class(level: usize) -> String {
    format!("title-{level}")
}

fn escape_markup(value: &str) -> Cow<'_, str> {
    if value
        .bytes()
        .any(|b| matches!(b, b'<' | b'>' | b'&' | b'"' | b'\''))
    {
        Cow::Owned(glib::markup_escape_text(value).to_string())
    } else {
        Cow::Borrowed(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_heading_levels_to_title_class() {
        assert_eq!(heading_css_class(1), "title-1");
        assert_eq!(heading_css_class(2), "title-2");
    }

    #[test]
    fn escapes_markup_in_text_labels() {
        assert_eq!(
            styled_text_markup("<unsafe>", Emphasis::Bold, InlineStyle::Normal),
            "<b>&lt;unsafe&gt;</b>"
        );
    }

    #[test]
    fn combines_nested_emphasis() {
        assert_eq!(combine_emphasis(Emphasis::Bold, Emphasis::Italic), Emphasis::BoldItalic);
        assert_eq!(combine_emphasis(Emphasis::Italic, Emphasis::Bold), Emphasis::BoldItalic);
        assert_eq!(combine_emphasis(Emphasis::Normal, Emphasis::Bold), Emphasis::Bold);
        assert_eq!(combine_emphasis(Emphasis::Bold, Emphasis::Normal), Emphasis::Bold);
        assert_eq!(combine_emphasis(Emphasis::BoldItalic, Emphasis::Italic), Emphasis::BoldItalic);
    }
}

// Render-pipeline test. Builds real GTK widgets and requires a display
// (X11, Wayland, or Xvfb). #[ignore]d by default; opt in with
// `cargo test -- --ignored`. GTK pins itself to one thread, so all
// rendering assertions live in a single test function.
#[cfg(test)]
mod render_pipeline_tests {
    use crate::MarkdownTextView;
    use gtk::prelude::*;

    fn child_types(view: &MarkdownTextView) -> Vec<String> {
        let container: &gtk::Box = view.upcast_ref();
        let mut types = Vec::new();
        let mut child = container.first_child();
        while let Some(c) = child {
            types.push(c.type_().name().to_string());
            child = c.next_sibling();
        }
        types
    }

    #[test]
    #[ignore]
    fn renders_each_block_kind() {
        gtk::init().expect("gtk::init");

        let para = MarkdownTextView::new();
        para.set_markdown("Hello *world*".to_string());
        assert_eq!(child_types(&para), vec!["GtkFlowBox"]);

        let hr = MarkdownTextView::new();
        hr.set_markdown("---".to_string());
        assert_eq!(child_types(&hr), vec!["GtkSeparator"]);

        let list = MarkdownTextView::new();
        list.set_markdown("- a\n- b\n- c".to_string());
        assert_eq!(child_types(&list), vec!["GtkBox"]);

        let code = MarkdownTextView::new();
        code.set_markdown("```\nfn x() {}\n```".to_string());
        assert_eq!(child_types(&code), vec!["GtkFrame"]);

        let heading = MarkdownTextView::new();
        heading.set_markdown("# Title".to_string());
        assert_eq!(child_types(&heading), vec!["GtkFlowBox"]);

        let quote = MarkdownTextView::new();
        quote.set_markdown("> quoted".to_string());
        assert_eq!(child_types(&quote), vec!["GtkFlowBox"]);
    }
}
