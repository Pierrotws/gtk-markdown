## 0.2.0 — 2026-05-01

### Added

- `link-activated` GObject signal that fires on Markdown link clicks;
  return `true` to inhibit the default URI launcher. The signal uses a
  first-true-wins accumulator, so the earliest connected handler that
  returns `true` stops further handlers and the default action.
- `MarkdownTextView::set_base_path` to resolve relative image URIs
  against an explicit base directory.
- `markdown` and `heading-level-offset` exposed as GObject properties
  (settable from `.ui` / GtkBuilder, observable via `notify::*`).
- Public parser AST: `parser::{MarkdownBlock, InlineSegment, Emphasis}`
  plus `markdown_blocks` / `parse_inline_segments` for downstream
  renderers.
- `.github/workflows/ci.yml` — runs `cargo clippy --all-targets -D
  warnings`, `cargo test --all-targets`, and `cargo doc --no-deps -D
  warnings` on each push and PR.
- Integration test (`tests/link_activated.rs`) covering the
  `link-activated` accumulator semantics and the activate-link
  trampoline end-to-end.
- Minimum size (64 × 64 px) for embedded pictures via the same CSS
  provider that pins `max-height`. Tiny images no longer shrink below
  a readable size when the container narrows.

### Changed

- Inline `` `code` `` now renders as a pure-Pango span (monospace,
  gray background, white foreground) inside the surrounding paragraph
  Label, so it wraps with the rest of the text instead of being
  hoisted into a separate framed FlowBox child. Code blocks
  (triple-backtick) keep the `gtk::Frame` + `gtk::Label` rendering.

### Fixed

- Reference cycle in the link-activated closure that pinned the View
  (and every child widget) until the next `set_markdown` cleared the
  children. The closure now holds a `WeakRef`.
- In-flight image loaders no longer call `set_paintable` on orphaned
  `gtk::Picture`s after a subsequent `set_markdown` rebuild. Each
  loader captures a generation counter at spawn time and bails if the
  View has rebuilt by the time decode finishes.
- Horizontal rules now have a 6 px top/bottom margin and no longer
  read as a hairline jammed into adjacent prose.

## 0.1.0

Initial release: GTK4 `MarkdownTextView` widget rendering paragraphs,
ATX headings, `>` quotes, unordered (`-`/`*`/`+`) and ordered (`N.`)
list items, fenced code blocks, inline code, `[label](uri)` links,
`![alt](src)` images (async load, max-height cap), thematic breaks,
and `*`/`_`/`**`/`__`/`***`/`___` emphasis.
