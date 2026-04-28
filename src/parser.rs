//! Markdown source → block / inline token parsing.
//!
//! The parser is intentionally small: it covers paragraphs, ATX headings,
//! `>` quotes, unordered (`-`/`*`/`+`) and ordered (`N.`) list items, fenced
//! `\`\`\`` code blocks, inline code, `[label](uri)` links, and bold/italic
//! emphasis with `*`/`_`/`**`/`__`/`***`/`___`. Soft newlines inside a
//! paragraph collapse to spaces.

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum MarkdownBlock {
    Paragraph(String),
    Heading { level: usize, text: String },
    Quote(String),
    UnorderedListItem(String),
    OrderedListItem { marker: String, text: String },
    Code(String),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum InlineSegment<'a> {
    Text(&'a str),
    Styled { text: &'a str, emphasis: Emphasis },
    Code(&'a str),
    Link { label: &'a str, uri: &'a str },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Emphasis {
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

pub(crate) fn markdown_blocks(value: &str) -> Vec<MarkdownBlock> {
    let mut blocks = Vec::new();
    let mut paragraph = String::new();
    let mut code_block = String::new();
    let mut in_code_block = false;

    for line in value.lines() {
        let trimmed = line.trim_start();

        if trimmed.starts_with("```") {
            if in_code_block {
                blocks.push(MarkdownBlock::Code(std::mem::take(&mut code_block)));
                in_code_block = false;
            } else {
                flush_paragraph(&mut blocks, &mut paragraph);
                in_code_block = true;
            }
            continue;
        }

        if in_code_block {
            if !code_block.is_empty() {
                code_block.push('\n');
            }
            code_block.push_str(line);
        } else {
            if line.trim().is_empty() {
                flush_paragraph(&mut blocks, &mut paragraph);
                continue;
            }

            if let Some((level, heading)) = parse_heading(trimmed) {
                flush_paragraph(&mut blocks, &mut paragraph);
                blocks.push(MarkdownBlock::Heading {
                    level,
                    text: heading.trim().to_string(),
                });
                continue;
            }

            if let Some(quote) = trimmed.strip_prefix("> ") {
                flush_paragraph(&mut blocks, &mut paragraph);
                blocks.push(MarkdownBlock::Quote(quote.trim().to_string()));
                continue;
            }

            if let Some(item) = parse_unordered_list_item(trimmed) {
                flush_paragraph(&mut blocks, &mut paragraph);
                blocks.push(MarkdownBlock::UnorderedListItem(item.trim().to_string()));
                continue;
            }

            if let Some((number, item)) = parse_ordered_list_item(trimmed) {
                flush_paragraph(&mut blocks, &mut paragraph);
                blocks.push(MarkdownBlock::OrderedListItem {
                    marker: format!("{number}."),
                    text: item.trim().to_string(),
                });
                continue;
            }

            if !paragraph.is_empty() {
                paragraph.push(' ');
            }
            paragraph.push_str(line.trim());
        }
    }

    if in_code_block {
        blocks.push(MarkdownBlock::Code(code_block));
    }
    flush_paragraph(&mut blocks, &mut paragraph);

    blocks
}

fn flush_paragraph(blocks: &mut Vec<MarkdownBlock>, paragraph: &mut String) {
    if paragraph.is_empty() {
        return;
    }

    blocks.push(MarkdownBlock::Paragraph(std::mem::take(paragraph)));
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

pub(crate) fn parse_inline_segments(value: &str) -> Vec<InlineSegment<'_>> {
    let mut segments = Vec::new();
    let mut index = 0;

    while index < value.len() {
        let rest = &value[index..];

        if let Some((token_len, inner, consumed, emphasis)) = parse_emphasis(
            rest,
            &[("___", Emphasis::BoldItalic), ("***", Emphasis::BoldItalic)],
        ) {
            segments.push(InlineSegment::Styled {
                text: inner,
                emphasis,
            });
            index += token_len + consumed + token_len;
            continue;
        }

        if let Some((token_len, inner, consumed, emphasis)) =
            parse_emphasis(rest, &[("__", Emphasis::Bold), ("**", Emphasis::Bold)])
        {
            segments.push(InlineSegment::Styled {
                text: inner,
                emphasis,
            });
            index += token_len + consumed + token_len;
            continue;
        }

        if let Some((token_len, inner, consumed, emphasis)) =
            parse_emphasis(rest, &[("_", Emphasis::Italic), ("*", Emphasis::Italic)])
        {
            segments.push(InlineSegment::Styled {
                text: inner,
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

        if let Some((label, uri, consumed)) = parse_link(rest) {
            segments.push(InlineSegment::Link { label, uri });
            index += consumed;
            continue;
        }

        let next_special = rest
            .char_indices()
            .skip(1)
            .find_map(|(offset, character)| {
                matches!(character, '*' | '_' | '`' | '[').then_some(offset)
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

        let inner_end = value[inner_start..].find(token)? + inner_start;
        if inner_end == inner_start {
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
    let label_end = value.strip_prefix('[')?.find("](")? + 1;
    let uri_start = label_end + 2;
    let uri_end = value[uri_start..].find(')')? + uri_start;
    let label = &value[1..label_end];
    let uri = &value[uri_start..uri_end];

    if label.is_empty() || uri.is_empty() {
        return None;
    }

    Some((label, uri, uri_end + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_marker_emphasis() {
        assert_eq!(
            parse_inline_segments("*italic* _also italic_ __bold__ ___both___"),
            vec![
                InlineSegment::Styled {
                    text: "italic",
                    emphasis: Emphasis::Italic,
                },
                InlineSegment::Text(" "),
                InlineSegment::Styled {
                    text: "also italic",
                    emphasis: Emphasis::Italic,
                },
                InlineSegment::Text(" "),
                InlineSegment::Styled {
                    text: "bold",
                    emphasis: Emphasis::Bold,
                },
                InlineSegment::Text(" "),
                InlineSegment::Styled {
                    text: "both",
                    emphasis: Emphasis::BoldItalic,
                },
            ]
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
                MarkdownBlock::UnorderedListItem("item".into()),
                MarkdownBlock::OrderedListItem {
                    marker: "2.".into(),
                    text: "next".into(),
                },
            ]
        );
    }
}
