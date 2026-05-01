//! Block / inline tokens → GTK widgets.

use std::path::Path;

use gtk::{glib, prelude::*};

use crate::parser::{
    markdown_blocks, parse_inline_segments, Emphasis, InlineSegment, InlineStyle, MarkdownBlock,
};

pub(crate) fn render_into(
    container: &gtk::Box,
    value: &str,
    heading_level_offset: u32,
    base_path: Option<&Path>,
) {
    for block in markdown_blocks(value) {
        match block {
            MarkdownBlock::Paragraph(text) => {
                container.append(&inline_flow(&text, InlineStyle::Normal, None, base_path));
            }
            MarkdownBlock::Heading { level, text } => {
                let css_level = level.saturating_add(heading_level_offset as usize);
                container.append(&inline_flow(
                    &text,
                    InlineStyle::Heading(css_level),
                    None,
                    base_path,
                ));
            }
            MarkdownBlock::Quote(text) => {
                container.append(&inline_flow(&text, InlineStyle::Quote, None, base_path));
            }
            MarkdownBlock::List { ordered, start, items } => {
                container.append(&list_box(ordered, start, &items, base_path));
            }
            MarkdownBlock::Code(code) => container.append(&code_block_frame(&code)),
        }
    }
}

fn list_box(ordered: bool, start: u32, items: &[String], base_path: Option<&Path>) -> gtk::Box {
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    for (offset, item) in items.iter().enumerate() {
        let marker = if ordered {
            format!("{}.", start + offset as u32)
        } else {
            "•".to_string()
        };
        outer.append(&inline_flow(item, InlineStyle::Normal, Some(&marker), base_path));
    }
    outer
}

fn inline_flow(
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
        &flow,
        parse_inline_segments(text),
        Emphasis::Normal,
        style,
        base_path,
    );

    flow
}

fn render_inline_segments(
    flow: &gtk::FlowBox,
    segments: Vec<InlineSegment<'_>>,
    base_emphasis: Emphasis,
    style: InlineStyle,
    base_path: Option<&Path>,
) {
    for segment in segments {
        match segment {
            InlineSegment::Text(text) => append_text_segment(flow, text, base_emphasis, style),
            InlineSegment::Styled { children, emphasis } => {
                let composed = combine_emphasis(base_emphasis, emphasis);
                render_inline_segments(flow, children, composed, style, base_path);
            }
            InlineSegment::Code(text) => flow.insert(&inline_code_frame(text), -1),
            InlineSegment::Link { label, uri } => flow.insert(&link_label(label, uri, style), -1),
            InlineSegment::Image { alt, src } => match picture_from_src(src, base_path) {
                Some(picture) => flow.insert(&picture, -1),
                None => flow.insert(&image_fallback_label(alt), -1),
            },
        }
    }
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

fn picture_from_src(src: &str, base_path: Option<&Path>) -> Option<gtk::Picture> {
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
    let picture = gtk::Picture::for_filename(&resolved);
    picture.set_can_shrink(true);
    #[allow(deprecated)]
    picture.set_keep_aspect_ratio(true);
    picture.add_css_class(PICTURE_CSS_CLASS);
    Some(picture)
}

fn install_picture_css_provider() {
    use std::sync::OnceLock;
    static INSTALLED: OnceLock<()> = OnceLock::new();
    INSTALLED.get_or_init(|| {
        let provider = gtk::CssProvider::new();
        #[allow(deprecated)]
        provider.load_from_data(&format!(
            ".{PICTURE_CSS_CLASS} {{ max-height: {MAX_PICTURE_HEIGHT_PX}px; }}"
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

fn append_text_segment(flow: &gtk::FlowBox, text: &str, emphasis: Emphasis, style: InlineStyle) {
    if let Some(text) = display_text_segment(text) {
        flow.insert(&text_label(text, emphasis, style), -1);
    }
}

fn display_text_segment(text: &str) -> Option<&str> {
    let text = text.trim();
    (!text.is_empty()).then_some(text)
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
    let escaped = apply_emphasis_markup(&escaped, emphasis);

    style_markup(escaped, style)
}

fn style_markup(markup: String, style: InlineStyle) -> String {
    match style {
        InlineStyle::Normal => markup,
        InlineStyle::Heading(_) => format!("<b>{markup}</b>"),
        InlineStyle::Quote => format!("<span style=\"italic\">{markup}</span>"),
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

fn inline_code_frame(text: &str) -> gtk::Frame {
    framed_widget(&code_flow(text), false, 2, 6)
}

fn code_block_frame(text: &str) -> gtk::Frame {
    let block = gtk::Box::new(gtk::Orientation::Vertical, 0);
    block.set_hexpand(true);

    for line in text.lines() {
        block.append(&code_flow(line));
    }

    if text.is_empty() {
        block.append(&code_flow(""));
    }

    framed_widget(&block, true, 8, 8)
}

fn framed_widget<W>(
    child: &W,
    hexpand: bool,
    vertical_margin: i32,
    horizontal_margin: i32,
) -> gtk::Frame
where
    W: IsA<gtk::Widget>,
{
    child.set_margin_top(vertical_margin);
    child.set_margin_bottom(vertical_margin);
    child.set_margin_start(horizontal_margin);
    child.set_margin_end(horizontal_margin);

    let frame = gtk::Frame::new(None);
    frame.set_hexpand(hexpand);
    frame.set_child(Some(child));
    frame
}

fn code_flow(text: &str) -> gtk::FlowBox {
    let flow = gtk::FlowBox::new();
    flow.set_selection_mode(gtk::SelectionMode::None);
    flow.set_homogeneous(false);
    flow.set_column_spacing(0);
    flow.set_row_spacing(0);
    flow.set_max_children_per_line(1000);
    flow.insert(&code_label(text), -1);

    flow
}

fn code_label(text: &str) -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_selectable(true);
    label.set_use_markup(true);
    label.set_markup(&format!(
        "<span font_family=\"monospace\">{}</span>",
        escape_markup(text)
    ));
    label
}

fn link_label(label: &str, uri: &str, style: InlineStyle) -> gtk::Label {
    let link = format!(
        "<a href=\"{}\">{}</a>",
        escape_markup(uri),
        escape_markup(label)
    );

    let label = gtk::Label::new(None);
    label.set_xalign(0.0);
    label.set_selectable(true);
    if let InlineStyle::Heading(level) = style {
        label.add_css_class(&heading_css_class(level));
    }
    label.set_use_markup(true);
    label.set_markup(&style_markup(link, style));
    label
}

fn heading_css_class(level: usize) -> String {
    format!("title-{level}")
}

fn escape_markup(value: &str) -> String {
    glib::markup_escape_text(value).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_same_style_words_in_one_text_label() {
        assert_eq!(display_text_segment("This is "), Some("This is"));
        assert_eq!(display_text_segment(" new example"), Some("new example"));
        assert_eq!(display_text_segment(" "), None);
    }

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
