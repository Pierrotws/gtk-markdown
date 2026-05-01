# gtk-markdown — Code Review Report

A full review of the crate. Findings are grouped by area and tagged with
severity:

- **Critical** — incorrect output for plausible inputs, or a safety hazard.
- **High** — visible bug or significant API/architecture concern that will
  bite users.
- **Medium** — quality issue worth fixing soon.
- **Low** — polish, micro-optimization, or future-proofing.

Section numbers from the original review are preserved; resolved items are
purged from this report as they land. Commit history carries the details.

## Tooling status

- `cargo test` — 23/23 pass.
- `cargo clippy --all-targets` — clean at the default lint level.
- `cargo clippy -- -W clippy::pedantic` — 5 warnings (cosmetic; itemized in
  §6.4).
- `cargo doc --no-deps` — builds without warnings.
- `cargo build --example window` — clean.

---

## 1. Correctness — parser

### 1.6 Empty alt text rejects valid images — **Low**
**Location:** `src/parser.rs:269–281` (via `parse_link`)

`parse_link` returns `None` when `label.is_empty()`. `parse_image` reuses
that, so `![](logo.png)` — a perfectly valid image with empty alt — falls
through to text. Pure-decorative images can't be expressed.

**Fix:** in `parse_image`, allow empty alt; only `parse_link` should keep
the empty-label rejection (an empty link label is semantically odd).

### 1.7 Indented code-fence inside a code block — **Low**
**Location:** `src/parser.rs:51–60`

```rust
let trimmed = line.trim_start();
if trimmed.starts_with("```") { ... }
```

The check runs *before* `if in_code_block { ... }`, and uses `trim_start`.
A code-block line that happens to begin with whitespace + ``` will close
the fence. CommonMark only closes on an *unindented* (≤3 spaces) match.

**Fix:** when `in_code_block`, only close on an unindented (or matching
indent) ` ``` `.

### 1.8 No setext headings, hr, autolinks, tables, strikethrough, ref-style links — **Low**
**Location:** parser overall.

Documented as out-of-scope in the Cargo description ("subset of Markdown"),
but worth surfacing for users picking this crate. Notably **horizontal
rules** (`---` / `***`) silently render as a paragraph of literal
asterisks/hyphens.

---

## 2. Correctness — renderer

### 2.2 FlowBox is being used as a text-flow container — **Medium / Architecture**
**Location:** `src/render.rs:33–57`

`gtk::FlowBox` arranges children in a *grid* (children fit horizontally up
to `max_children_per_line`, then wrap to a new row). Each inline run is a
separate child, so:

- **Word spacing is `column_spacing(4)`** instead of natural Pango
  inter-word spacing — close to one ASCII space at 11pt sans-serif, but
  doesn't track font size or the actual space-glyph width.
- **Selection cannot cross children.** Each emphasis run / link / code
  span is its own selectable label; users can't select a phrase that
  spans `*foo* bar`.
- **Wrapping happens at child boundaries**, not within paragraphs, so a
  very long single-segment paragraph and one with many small segments
  wrap differently for the same visual width.
- **Justification, hyphenation, `text-wrap` modes** — none of these can
  be applied across the inline run.

This is a deliberate-looking decision (it lets framed inline code and
images live alongside text), but it puts a ceiling on text quality. The
common alternative is "build one Pango-marked-up `gtk::Label` per run of
homogeneous text/links/emphasis, only break out widgets for things that
genuinely need to be widgets (framed code, images)."

**Fix:** for emphasis-only and link-only spans, accumulate Pango markup
into a single `Label` and only insert separate widgets for `Code` and
`Image` segments. Use a horizontal `Box` with children that are mostly
single-paragraph Labels to preserve in-paragraph wrapping inside Pango.

### 2.7 Heading link styling has a bug, code links go unstyled — **Low**
**Location:** `src/render.rs:169–185`

`link_label` applies `style_markup` to the `<a>...</a>` markup, which for
`InlineStyle::Heading` wraps it in `<b>`. That works. But `style_markup`
does *not* reapply the `title-N` CSS class wrapping — and inside a code
context (none yet) this would silently drop styling. More concretely:
heading links get bolded but don't inherit the heading font *size* unless
the CSS class is set on the same Label, which the function does do
(`label.add_css_class(&heading_css_class(level))`). OK in practice; flagged
because the relationship between Pango markup and CSS classes is fragile.

**Fix (long-term):** unify the styling path so that "this label belongs to
heading level N" is a single decision, not duplicated between
`add_css_class` and `style_markup`.

### 2.8 `style_markup` returns `String` for the `Normal` no-op — **Low**
**Location:** `src/render.rs:90–96`

```rust
match style {
    InlineStyle::Normal => markup,
    ...
}
```

`markup` is already a `String`; the function takes ownership and returns
it back in the `Normal` arm. Fine, just allocating one extra `String`
unnecessarily in the call chain. `Cow<str>` would be cleaner. Micro.

---

## 3. API design

### 3.2 `set_heading_level_offset` rebuilds even when markdown is empty — **Low**
**Location:** `src/lib.rs:53–60`

```rust
pub fn set_heading_level_offset(&self, offset: u32) {
    if self.imp().heading_level_offset.get() == offset { return; }
    self.imp().heading_level_offset.set(offset);
    let text = self.markdown();
    self.set_markdown(&text);
}
```

When `text` is empty, `set_markdown("")` still does
`clear_box` + `render_into("")`. Cheap but needless.

**Fix:** skip the rebuild if `markdown.borrow().is_empty()`.

### 3.4 No way to override link click behaviour — **Low**

Links rely on `gtk::Label`'s default `activate-link` handler, which goes
through `gio::AppInfo::launch_default_for_uri`. Apps embedding this widget
inside a wiki-like environment can't intercept clicks (e.g., to navigate
in-app for `app://...` URIs).

**Fix:** expose a public `connect_link_activated` style signal (or wrap
each link's `activate-link` and forward).

### 3.5 No accessor for the parsed AST — **Low**

The internal `parser` module is `pub(crate)`. Consumers who want to render
their own way (or operate on the AST) have to re-parse with a different
crate. Worth considering whether `MarkdownBlock` and `InlineSegment` should
be public.

---

## 4. Performance

### 4.1 N labels per paragraph — **Medium**

Already covered in §2.2. Each emphasis/link/code segment instantiates a
fresh `gtk::Label` (sometimes wrapped in a `gtk::Frame`). For prose-heavy
documents that's tens of widgets per paragraph, hundreds per page.

### 4.3 `escape_markup` always allocates — **Low**
**Location:** `src/render.rs:191–193`

`glib::markup_escape_text` returns a `GString`; the helper unconditionally
clones it via `.to_string()`. Fine, but a frequently-hit allocation.

---

## 5. Test coverage

### 5.2 No render-pipeline tests — **Low**

All render tests are micro-tests of helper functions
(`display_text_segment`, `heading_css_class`, `styled_text_markup`).
End-to-end tests that build a `MarkdownTextView` and inspect its widget
tree are possible (gtk-rs supports headless testing via
`gtk::init` in a test binary on a system with X/Wayland or `Xvfb`), but
not a free-roll. At minimum, add a test that calls
`render_into(&fake_box, "...")` and asserts the resulting child types.

### 5.3 No tests for edge cases — **Low**

Suggested additions:

- CRLF line endings (`value.lines()` already strips `\r\n`, so probably
  fine, but worth pinning).
- Empty input.
- A code block at end-of-input without a closing fence.
- An unmatched emphasis delimiter (`**foo`).
- A heading with inline emphasis (`# Hello *world*`).

---

## 6. Polish & style

### 6.1 Missing `#[must_use]` on accessors — **Low**
**Location:** `src/lib.rs:27, 37, 48` (clippy `must_use_candidate`)

`new()`, `markdown()`, `heading_level_offset()` are pure value-returning
methods.

### 6.2 Missing `;` on a `match` arm — **Low**
**Location:** `src/render.rs:49` (clippy `semicolon_if_nothing_returned`)

```rust
InlineSegment::Styled { text, emphasis } => {
    append_text_segment(&flow, text, emphasis, style)
},
```

The trailing expression has unit type — convention is to terminate with
`;` so the arm is a statement.

### 6.3 Doc-comment backtick parity — **Low**
**Location:** `src/parser.rs:3–7` (clippy `doc_markdown`)

The fenced-code mention `\`\`\`` confuses clippy's parser. Either escape
differently or `#[allow(clippy::doc_markdown)]` on the module.

### 6.4 Magic numbers in the renderer — **Low**

`set_max_children_per_line(1000)`, `set_column_spacing(4)`,
`set_row_spacing(2)`, frame margins of `2` / `6` / `8`. None are
self-describing and a reader has to infer intent. A short named constant
or one-line comment for each would help. (Per the project's "comments
only when WHY is non-obvious" rule, the column-spacing-vs-Pango-spacing
choice in particular deserves a `// Approximates a space at 11pt` note.)

### 6.5 `Cargo.toml` — no `rust-version` — **Low**

Declaring an MSRV avoids surprise breakage for downstream users on older
toolchains.

### 6.6 README install snippet uses git dep — **Low**
**Location:** `README.md` "Installation" section.

Once published, `gtk-markdown = "0.1"` is friendlier than a `git = ...`
dependency.

---

## 7. Recommended priority order

Remaining work, roughly in the order I'd tackle it:

1. **§2.2 — Inline rendering rewrite (FlowBox → Pango Labels).**
   Biggest visual-quality lever. Subsumes §4.1.
2. **§5.3 — Fill obvious test gaps** (end-of-input edge cases) — pure
   addition, near-zero risk.
3. Everything else (polish, micro-perf, optional features).

---

## 8. What works well

To balance the above, the things this codebase already does right:

- **Module boundaries are clean.** `parser` produces tokens, `render`
  consumes them, `imp` glues to GObject. Each module has a single
  responsibility and the tests live next to the code under test.
- **GObject subclass setup is correct.** The wrapper macro, the
  `ObjectSubclass` impl, the `BoxImpl`/`WidgetImpl` empty impls, and the
  `constructed` orientation set are all idiomatic.
- **Pango-markup escaping is consistent.** Every place that builds markup
  goes through `escape_markup` — no XSS-style hole through a stray
  formatter.
- **Tests are tight and readable.** They assert structural equality of
  `Vec<MarkdownBlock>` / `Vec<InlineSegment>`, which is exactly the right
  level of granularity for a parser of this size.
- **The `heading_level_offset` knob is a thoughtful touch.** Embedding the
  view inside a container that already styles its children as a heading is
  a real use case, and the offset solves it without forcing the user to
  rewrite their markdown.
- **The image fallback is graceful.** Missing files / remote URLs degrade
  to an italic `[image: alt]` placeholder rather than silently failing or
  panicking.
- **No `unwrap`/`expect`/`panic!` on input paths.** The parser handles
  malformed input by emitting plain text, which is the right behaviour
  for a "best-effort renderer."

---

*Generated for the state of `main` as of this review.*
