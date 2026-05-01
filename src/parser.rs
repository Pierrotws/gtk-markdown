//! Markdown source → block / inline token parsing.
//!
//! The parser is intentionally small: it covers paragraphs, ATX headings,
//! `>` quotes, unordered (`-`/`*`/`+`) and ordered (`N.`) list items, fenced
//! code blocks (triple-backtick), inline code, `[label](uri)` links,
//! `![alt](src)` images, thematic breaks (`---` / `***` / `___`), and
//! bold/italic emphasis with `*`, `_`, `**`, `__`, `***`, `___`. Soft
//! newlines inside a paragraph collapse to spaces.
//!
//! Out of scope (currently): setext headings, autolinks, tables,
//! strikethrough, reference-style links, HTML blocks, nested lists.

/// Block-level token produced by [`markdown_blocks`].
#[derive(Debug, PartialEq, Eq)]
pub enum MarkdownBlock {
    Paragraph(String),
    Heading { level: usize, text: String },
    Quote(String),
    List {
        ordered: bool,
        start: u32,
        items: Vec<String>,
    },
    Code(String),
    HorizontalRule,
}

/// Inline-level token produced by [`parse_inline_segments`]. Borrows from
/// the input string.
#[derive(Debug, PartialEq, Eq)]
pub enum InlineSegment<'a> {
    Text(&'a str),
    Styled {
        children: Vec<InlineSegment<'a>>,
        emphasis: Emphasis,
    },
    Code(&'a str),
    Link { label: &'a str, uri: &'a str },
    Image { alt: &'a str, src: &'a str },
}

/// Emphasis applied to a [`InlineSegment::Styled`] run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Emphasis {
    Normal,
    Italic,
    Bold,
    BoldItalic,
}

#[derive(Clone, Copy)]
pub(crate) enum InlineStyle {
    Normal,
    Heading(usize),
    Quote,
}

struct PendingList {
    ordered: bool,
    start: u32,
    items: Vec<String>,
}

/// Parses Markdown source into a flat list of block-level tokens.
#[must_use]
pub fn markdown_blocks(value: &str) -> Vec<MarkdownBlock> {
    let mut blocks = Vec::new();
    let mut paragraph = String::new();
    let mut quote = String::new();
    let mut list: Option<PendingList> = None;
    let mut code_block = String::new();
    let mut in_code_block = false;

    for line in value.lines() {
        let trimmed = line.trim_start();
        let leading_ws = line.len() - trimmed.len();
        let unindented = leading_ws <= 3;

        if in_code_block {
            if unindented && trimmed.starts_with("```") {
                blocks.push(MarkdownBlock::Code(std::mem::take(&mut code_block)));
                in_code_block = false;
            } else {
                if !code_block.is_empty() {
                    code_block.push('\n');
                }
                code_block.push_str(line);
            }
            continue;
        }

        if unindented && trimmed.starts_with("```") {
            flush_paragraph(&mut blocks, &mut paragraph);
            flush_quote(&mut blocks, &mut quote);
            flush_list(&mut blocks, &mut list);
            in_code_block = true;
            continue;
        }

        if line.trim().is_empty() {
            flush_paragraph(&mut blocks, &mut paragraph);
            flush_quote(&mut blocks, &mut quote);
            flush_list(&mut blocks, &mut list);
            continue;
        }

        if let Some((level, heading)) = parse_heading(trimmed) {
            flush_paragraph(&mut blocks, &mut paragraph);
            flush_quote(&mut blocks, &mut quote);
            flush_list(&mut blocks, &mut list);
            blocks.push(MarkdownBlock::Heading {
                level,
                text: heading.trim().to_string(),
            });
            continue;
        }

        if unindented && is_horizontal_rule(trimmed) {
            flush_paragraph(&mut blocks, &mut paragraph);
            flush_quote(&mut blocks, &mut quote);
            flush_list(&mut blocks, &mut list);
            blocks.push(MarkdownBlock::HorizontalRule);
            continue;
        }

        if let Some(rest) = parse_quote_line(trimmed) {
            flush_paragraph(&mut blocks, &mut paragraph);
            flush_list(&mut blocks, &mut list);
            if !quote.is_empty() {
                quote.push(' ');
            }
            quote.push_str(rest.trim());
            continue;
        }

        if let Some(item) = parse_unordered_list_item(trimmed) {
            flush_paragraph(&mut blocks, &mut paragraph);
            flush_quote(&mut blocks, &mut quote);
            let item = item.trim().to_string();
            match list.as_mut() {
                Some(pending) if !pending.ordered => pending.items.push(item),
                _ => {
                    flush_list(&mut blocks, &mut list);
                    list = Some(PendingList {
                        ordered: false,
                        start: 1,
                        items: vec![item],
                    });
                }
            }
            continue;
        }

        if let Some((number, item)) = parse_ordered_list_item(trimmed) {
            flush_paragraph(&mut blocks, &mut paragraph);
            flush_quote(&mut blocks, &mut quote);
            let item = item.trim().to_string();
            let parsed_start: u32 = number.parse().unwrap_or(1);
            match list.as_mut() {
                Some(pending) if pending.ordered => pending.items.push(item),
                _ => {
                    flush_list(&mut blocks, &mut list);
                    list = Some(PendingList {
                        ordered: true,
                        start: parsed_start,
                        items: vec![item],
                    });
                }
            }
            continue;
        }

        flush_quote(&mut blocks, &mut quote);
        flush_list(&mut blocks, &mut list);
        if !paragraph.is_empty() {
            paragraph.push(' ');
        }
        paragraph.push_str(line.trim());
    }

    if in_code_block {
        blocks.push(MarkdownBlock::Code(code_block));
    }
    flush_paragraph(&mut blocks, &mut paragraph);
    flush_quote(&mut blocks, &mut quote);
    flush_list(&mut blocks, &mut list);

    blocks
}

fn flush_paragraph(blocks: &mut Vec<MarkdownBlock>, paragraph: &mut String) {
    if paragraph.is_empty() {
        return;
    }

    blocks.push(MarkdownBlock::Paragraph(std::mem::take(paragraph)));
}

fn flush_quote(blocks: &mut Vec<MarkdownBlock>, quote: &mut String) {
    if quote.is_empty() {
        return;
    }

    blocks.push(MarkdownBlock::Quote(std::mem::take(quote)));
}

fn flush_list(blocks: &mut Vec<MarkdownBlock>, list: &mut Option<PendingList>) {
    if let Some(pending) = list.take() {
        blocks.push(MarkdownBlock::List {
            ordered: pending.ordered,
            start: pending.start,
            items: pending.items,
        });
    }
}

fn parse_heading(line: &str) -> Option<(usize, &str)> {
    let level = line
        .chars()
        .take_while(|character| *character == '#')
        .count();
    if (1..=6).contains(&level) && line[level..].starts_with(' ') {
        Some((level, &line[level + 1..]))
    } else {
        None
    }
}

fn parse_quote_line(trimmed: &str) -> Option<&str> {
    let rest = trimmed.strip_prefix('>')?;
    Some(rest.strip_prefix(' ').unwrap_or(rest))
}

fn is_horizontal_rule(trimmed: &str) -> bool {
    let mut chars = trimmed.chars().filter(|c| !c.is_ascii_whitespace());
    let Some(first) = chars.next() else { return false };
    if !matches!(first, '-' | '*' | '_') {
        return false;
    }
    let mut count = 1;
    for c in chars {
        if c != first {
            return false;
        }
        count += 1;
    }
    count >= 3
}

fn parse_unordered_list_item(line: &str) -> Option<&str> {
    line.strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .or_else(|| line.strip_prefix("+ "))
}

fn parse_ordered_list_item(line: &str) -> Option<(&str, &str)> {
    let (number, rest) = line.split_once(". ")?;

    if !number.is_empty() && number.chars().all(|character| character.is_ascii_digit()) {
        Some((number, rest))
    } else {
        None
    }
}

/// Parses inline Markdown source (within a paragraph or other block) into
/// a list of inline tokens borrowing from `value`.
#[must_use]
pub fn parse_inline_segments(value: &str) -> Vec<InlineSegment<'_>> {
    let mut segments = Vec::new();
    let mut index = 0;

    while index < value.len() {
        let rest = &value[index..];

        if let Some(after_backslash) = rest.strip_prefix('\\') {
            if let Some(escaped) = after_backslash.chars().next() {
                if escaped.is_ascii_punctuation() {
                    segments.push(InlineSegment::Text(&value[index + 1..index + 2]));
                    index += 2;
                    continue;
                }
            }
        }

        if let Some((token_len, inner, consumed, emphasis)) = parse_emphasis(
            rest,
            &[("___", Emphasis::BoldItalic), ("***", Emphasis::BoldItalic)],
        ) {
            segments.push(InlineSegment::Styled {
                children: parse_inline_segments(inner),
                emphasis,
            });
            index += token_len + consumed + token_len;
            continue;
        }

        if let Some((token_len, inner, consumed, emphasis)) =
            parse_emphasis(rest, &[("__", Emphasis::Bold), ("**", Emphasis::Bold)])
        {
            segments.push(InlineSegment::Styled {
                children: parse_inline_segments(inner),
                emphasis,
            });
            index += token_len + consumed + token_len;
            continue;
        }

        if let Some((token_len, inner, consumed, emphasis)) =
            parse_emphasis(rest, &[("_", Emphasis::Italic), ("*", Emphasis::Italic)])
        {
            segments.push(InlineSegment::Styled {
                children: parse_inline_segments(inner),
                emphasis,
            });
            index += token_len + consumed + token_len;
            continue;
        }

        if let Some((token_len, inner, consumed)) = parse_wrapped(rest, &["`"]) {
            segments.push(InlineSegment::Code(inner));
            index += token_len + consumed + token_len;
            continue;
        }

        if let Some((alt, src, consumed)) = parse_image(rest) {
            segments.push(InlineSegment::Image { alt, src });
            index += consumed;
            continue;
        }

        if let Some((label, uri, consumed)) = parse_link(rest) {
            segments.push(InlineSegment::Link { label, uri });
            index += consumed;
            continue;
        }

        let next_special = rest
            .char_indices()
            .skip(1)
            .find_map(|(offset, character)| {
                matches!(character, '*' | '_' | '`' | '[' | '!' | '\\').then_some(offset)
            })
            .unwrap_or(rest.len());
        segments.push(InlineSegment::Text(&rest[..next_special]));
        index += next_special;
    }

    segments
}

fn parse_emphasis<'a>(
    value: &'a str,
    tokens: &[(&str, Emphasis)],
) -> Option<(usize, &'a str, usize, Emphasis)> {
    for (token, emphasis) in tokens {
        let inner_start = token.len();
        if !value.starts_with(token) {
            continue;
        }

        if value[inner_start..].starts_with(char::is_whitespace) {
            continue;
        }

        let inner_end = value[inner_start..].find(token)? + inner_start;
        if inner_end == inner_start {
            continue;
        }

        if value[..inner_end]
            .chars()
            .next_back()
            .is_some_and(char::is_whitespace)
        {
            continue;
        }

        return Some((
            token.len(),
            &value[inner_start..inner_end],
            inner_end - inner_start,
            *emphasis,
        ));
    }

    None
}

fn parse_wrapped<'a>(value: &'a str, tokens: &[&str]) -> Option<(usize, &'a str, usize)> {
    for token in tokens {
        let inner_start = token.len();
        if !value.starts_with(token) {
            continue;
        }

        let inner_end = value[inner_start..].find(token)? + inner_start;
        if inner_end == inner_start {
            continue;
        }

        return Some((
            token.len(),
            &value[inner_start..inner_end],
            inner_end - inner_start,
        ));
    }

    None
}

fn parse_link(value: &str) -> Option<(&str, &str, usize)> {
    let (label, uri, consumed) = parse_link_like(value)?;
    if label.is_empty() {
        return None;
    }
    Some((label, uri, consumed))
}

fn parse_link_like(value: &str) -> Option<(&str, &str, usize)> {
    let label_end = value.strip_prefix('[')?.find("](")? + 1;
    let uri_start = label_end + 2;
    let uri_end = balanced_close_paren_offset(&value[uri_start..])? + uri_start;
    let label = &value[1..label_end];
    let uri = &value[uri_start..uri_end];
    if uri.is_empty() {
        return None;
    }
    Some((label, uri, uri_end + 1))
}

fn balanced_close_paren_offset(value: &str) -> Option<usize> {
    let mut depth: u32 = 1;
    for (offset, ch) in value.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_image(value: &str) -> Option<(&str, &str, usize)> {
    let rest = value.strip_prefix('!')?;
    let (alt, src, consumed) = parse_link_like(rest)?;
    Some((alt, src, consumed + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_child(text: &str) -> InlineSegment<'_> {
        InlineSegment::Text(text)
    }

    fn styled(text: &str, emphasis: Emphasis) -> InlineSegment<'_> {
        InlineSegment::Styled {
            children: vec![text_child(text)],
            emphasis,
        }
    }

    #[test]
    fn parses_marker_emphasis() {
        assert_eq!(
            parse_inline_segments("*italic* _also italic_ __bold__ ___both___"),
            vec![
                styled("italic", Emphasis::Italic),
                InlineSegment::Text(" "),
                styled("also italic", Emphasis::Italic),
                InlineSegment::Text(" "),
                styled("bold", Emphasis::Bold),
                InlineSegment::Text(" "),
                styled("both", Emphasis::BoldItalic),
            ]
        );
    }

    #[test]
    fn parses_images_distinct_from_links() {
        assert_eq!(
            parse_inline_segments("see ![logo](path/to/logo.png) and [site](https://example.invalid)"),
            vec![
                InlineSegment::Text("see "),
                InlineSegment::Image {
                    alt: "logo",
                    src: "path/to/logo.png",
                },
                InlineSegment::Text(" and "),
                InlineSegment::Link {
                    label: "site",
                    uri: "https://example.invalid",
                },
            ]
        );
    }

    #[test]
    fn bare_bang_is_kept_as_text() {
        assert_eq!(
            parse_inline_segments("Hello! World"),
            vec![InlineSegment::Text("Hello"), InlineSegment::Text("! World")]
        );
    }

    #[test]
    fn parses_inline_code_and_links_as_widget_segments() {
        assert_eq!(
            parse_inline_segments("open `code` then [site](https://example.invalid)"),
            vec![
                InlineSegment::Text("open "),
                InlineSegment::Code("code"),
                InlineSegment::Text(" then "),
                InlineSegment::Link {
                    label: "site",
                    uri: "https://example.invalid",
                },
            ]
        );
    }

    #[test]
    fn parses_fenced_code_as_block() {
        assert_eq!(
            markdown_blocks("before\n```\na < b\nc > d\n```\nafter"),
            vec![
                MarkdownBlock::Paragraph("before".into()),
                MarkdownBlock::Code("a < b\nc > d".into()),
                MarkdownBlock::Paragraph("after".into()),
            ]
        );
    }

    #[test]
    fn merges_soft_newlines_into_paragraphs() {
        assert_eq!(
            markdown_blocks("hello\nworld\n\nnext paragraph"),
            vec![
                MarkdownBlock::Paragraph("hello world".into()),
                MarkdownBlock::Paragraph("next paragraph".into()),
            ]
        );
    }

    #[test]
    fn parses_structural_markdown_blocks() {
        assert_eq!(
            markdown_blocks("# Title\n- item\n2. next"),
            vec![
                MarkdownBlock::Heading {
                    level: 1,
                    text: "Title".into(),
                },
                MarkdownBlock::List {
                    ordered: false,
                    start: 1,
                    items: vec!["item".into()],
                },
                MarkdownBlock::List {
                    ordered: true,
                    start: 2,
                    items: vec!["next".into()],
                },
            ]
        );
    }

    #[test]
    fn merges_consecutive_quote_lines() {
        assert_eq!(
            markdown_blocks("> first line\n> second line\n\nparagraph"),
            vec![
                MarkdownBlock::Quote("first line second line".into()),
                MarkdownBlock::Paragraph("paragraph".into()),
            ]
        );
    }

    #[test]
    fn separates_quotes_split_by_blank_line() {
        assert_eq!(
            markdown_blocks("> a\n\n> b"),
            vec![
                MarkdownBlock::Quote("a".into()),
                MarkdownBlock::Quote("b".into()),
            ]
        );
    }

    #[test]
    fn groups_consecutive_unordered_list_items() {
        assert_eq!(
            markdown_blocks("- one\n- two\n- three"),
            vec![MarkdownBlock::List {
                ordered: false,
                start: 1,
                items: vec!["one".into(), "two".into(), "three".into()],
            }]
        );
    }

    #[test]
    fn ordered_and_unordered_lists_split() {
        assert_eq!(
            markdown_blocks("- a\n- b\n1. c\n2. d"),
            vec![
                MarkdownBlock::List {
                    ordered: false,
                    start: 1,
                    items: vec!["a".into(), "b".into()],
                },
                MarkdownBlock::List {
                    ordered: true,
                    start: 1,
                    items: vec!["c".into(), "d".into()],
                },
            ]
        );
    }

    #[test]
    fn backslash_escapes_punctuation() {
        assert_eq!(
            parse_inline_segments(r"\*not italic\*"),
            vec![
                InlineSegment::Text("*"),
                InlineSegment::Text("not italic"),
                InlineSegment::Text("*"),
            ]
        );
    }

    #[test]
    fn backslash_before_non_punctuation_is_kept() {
        assert_eq!(
            parse_inline_segments(r"a\b"),
            vec![InlineSegment::Text("a"), InlineSegment::Text(r"\b")]
        );
    }

    #[test]
    fn balanced_parens_inside_uri() {
        assert_eq!(
            parse_inline_segments("[link](https://example.com/path(1))"),
            vec![InlineSegment::Link {
                label: "link",
                uri: "https://example.com/path(1)",
            }]
        );
    }

    #[test]
    fn deeply_nested_parens_inside_uri() {
        assert_eq!(
            parse_inline_segments("[x](a(b(c)d)e)"),
            vec![InlineSegment::Link {
                label: "x",
                uri: "a(b(c)d)e",
            }]
        );
    }

    #[test]
    fn unbalanced_uri_paren_falls_back_to_text() {
        assert_eq!(
            parse_inline_segments("[link](http"),
            vec![InlineSegment::Text("[link](http")]
        );
    }

    #[test]
    fn whitespace_bounded_emphasis_is_text() {
        assert_eq!(
            parse_inline_segments("* not italic *"),
            vec![
                InlineSegment::Text("* not italic "),
                InlineSegment::Text("*"),
            ]
        );
    }

    #[test]
    fn trailing_whitespace_inside_emphasis_disqualifies() {
        assert_eq!(
            parse_inline_segments("*foo *"),
            vec![InlineSegment::Text("*foo "), InlineSegment::Text("*")]
        );
    }

    #[test]
    fn parse_image_with_empty_alt_text() {
        assert_eq!(
            parse_inline_segments("![](logo.png)"),
            vec![InlineSegment::Image { alt: "", src: "logo.png" }]
        );
    }

    #[test]
    fn parse_link_still_rejects_empty_label() {
        assert_eq!(
            parse_inline_segments("[](https://example.invalid)"),
            vec![InlineSegment::Text("[](https://example.invalid)")]
        );
    }

    #[test]
    fn indented_fence_inside_code_block_does_not_close() {
        assert_eq!(
            markdown_blocks("```\n    ```\n```"),
            vec![MarkdownBlock::Code("    ```".into())]
        );
    }

    #[test]
    fn horizontal_rule_dashes() {
        assert_eq!(
            markdown_blocks("---"),
            vec![MarkdownBlock::HorizontalRule]
        );
    }

    #[test]
    fn horizontal_rule_asterisks_with_spaces() {
        assert_eq!(
            markdown_blocks("* * *"),
            vec![MarkdownBlock::HorizontalRule]
        );
    }

    #[test]
    fn dash_with_text_remains_a_list_item() {
        assert_eq!(
            markdown_blocks("- item"),
            vec![MarkdownBlock::List {
                ordered: false,
                start: 1,
                items: vec!["item".into()],
            }]
        );
    }

    #[test]
    fn nested_emphasis_recurses() {
        assert_eq!(
            parse_inline_segments("**outer *inner* outer**"),
            vec![InlineSegment::Styled {
                children: vec![
                    InlineSegment::Text("outer "),
                    InlineSegment::Styled {
                        children: vec![InlineSegment::Text("inner")],
                        emphasis: Emphasis::Italic,
                    },
                    InlineSegment::Text(" outer"),
                ],
                emphasis: Emphasis::Bold,
            }]
        );
    }
}
