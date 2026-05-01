# gtk-markdown — Code Review Report

A fresh, top-to-bottom review of the crate as of `main` (18 commits ahead
of `origin/main`). Severity tags:

- **Critical** — incorrect output for plausible inputs, or a safety hazard.
- **High** — visible bug or significant API/architecture concern that will
  bite users.
- **Medium** — quality issue worth fixing soon.
- **Low** — polish, micro-optimization, or future-proofing.

This supersedes prior reviews; section numbers are renumbered. Findings
already addressed in commit history aren't repeated here.

## Tooling status

- `cargo test` — 33/33 pass (plus 1 `#[ignore]`d render-pipeline test
  opted in via `cargo test -- --ignored`; needs a display).
- `cargo clippy --all-targets` — clean at the default lint level.
- `cargo clippy -- -W clippy::pedantic` — 8 warnings, itemized in §6.
- `cargo doc --no-deps` — builds without warnings.
- `cargo build --example window` — clean.

The crate builds, runs, parses, and renders. The bug list below is mostly
quality-of-life.

---

## 1. Correctness

### 1.2 `connect_link_activated` accumulator is "last wins" — **Medium**
**Location:** `src/imp.rs:36–44` (signal definition), `src/lib.rs:73–82`.

The `link-activated` signal is built with the default GLib accumulator,
which returns the *last* connected handler's `bool`. That's fine for a
single subscriber (the typical case) but surprising for "intercept"
semantics: with two connected handlers, the second one's return value
wins regardless of the first's intent. Apps building modular routing
on top of this signal will hit confusing precedence bugs.

**Fix:** add a custom accumulator that stops at the first `true`:

```rust
Signal::builder("link-activated")
    .param_types([String::static_type()])
    .return_type::<bool>()
    .accumulator(|_hint, acc, value| {
        let handled = value.get::<bool>().unwrap_or(false);
        if handled {
            *acc = true.to_value();
            false   // stop further handlers
        } else {
            true    // keep going
        }
    })
    .build()
```

Document the precedence explicitly either way.

### 1.3 `parse_emphasis` greedy first-close still truncates triple-star runs — **Medium**
**Location:** `src/parser.rs:352–388`.

The parser finds the *first* matching closing token. For
`**bold and *italic***` the first `**` is the one inside `***` at the
end (the leading two of three `*`s), so the outer Bold consumes
`bold and *italic`, drops one `*`, and the trailing `*` becomes literal
text. CommonMark's "delimiter run" / left- and right-flanking algorithm
matches the *outermost* close that respects nesting; we can't get there
with a single greedy `find`.

This was deliberately left behind by §1.3 of the prior review (nested
emphasis recursion); flagging it again because it's the most common
remaining "this-isn't-italic-when-I-thought-it-would-be" surprise. A
simple workaround for the runs that matter most: if the input has more
`token`s than expected after an early close, retry with the next match.

**Fix:** real CommonMark delimiter runs (a non-trivial pass), or a small
heuristic: when a `**`/`__` close is the start of a longer run of the
same char, prefer the *last* `**` in that run.

### 1.5 Nested block quotes (`>>`) render as literal `>` — **Low**

`parse_quote_line` strips one `>` and one optional space. `>> nested`
becomes a Quote whose text is `> nested`, which the renderer prints
verbatim. CommonMark allows `>>` to mean a quote inside a quote.

**Fix:** parse quote depth and represent as `Quote(Vec<MarkdownBlock>)`
or `Quote { depth, text }`. Ties into the deeper "block-level recursion
inside quotes" rewrite.

### 1.6 Hard line breaks (trailing two-space) and `\` line breaks are not honoured — **Low**

CommonMark turns `foo  \nbar` (two trailing spaces) into a `<br>`. The
parser's per-line collapse to spaces drops trailing whitespace. Inline
backslash + newline does the same in CommonMark. Neither path is
implemented.

**Fix:** detect the trailing-double-space marker before `paragraph.push(' ')`
and emit something the renderer can interpret as a Pango `\n` inside
the combined Label.

### 1.7 Ordered-list `N)` form not accepted — **Low**

CommonMark allows both `1. foo` and `1) foo`. We only match `N. `.

### 1.8 List-item marker requires a single ASCII space — **Low**
**Location:** `src/parser.rs:250–253`.

```rust
fn parse_unordered_list_item(line: &str) -> Option<&str> {
    line.strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .or_else(|| line.strip_prefix("+ "))
}
```

A tab or multiple spaces after the marker (CommonMark accepts both)
falls through to a paragraph. Same for `parse_ordered_list_item`.

---

## 2. Renderer / widgets

### 2.1 Horizontal rule has no surrounding spacing — **Low**
**Location:** `src/render.rs:42–44`.

```rust
MarkdownBlock::HorizontalRule => {
    container.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
}
```

A `<hr>` rendered between two paragraphs touches both text blocks
because the View's outer Box has no inter-child spacing. Visually a
hairline jammed into adjacent prose.

**Fix:** either set a top/bottom margin on the Separator, or set
`spacing(N)` on the wrapper Box in `imp::constructed`.

### 2.2 `apply_emphasis_markup` always allocates — **Low**
**Location:** `src/render.rs:322–329`.

The `Emphasis::Normal` arm clones the input via `escaped.to_string()`
even though it's a pure no-op. With `escape_markup` already returning
`Cow<'_, str>` for the no-allocation case, doing the same here would
let prose paragraphs render entirely allocation-free between parser
and `Label::set_markup`.

**Fix:** return `Cow<'a, str>` and let `Normal` borrow.

### 2.3 `set_markdown_prop` and `rebuild` clone the text — **Low**
**Location:** `src/imp.rs:55–88`.

`set_markdown_prop(&self, text: String)` receives an owned `String`,
calls `self.rebuild(&text)`, and lets `rebuild` re-allocate via
`text.to_string()` to write into the `RefCell`. The original `String`
is dropped without being used.

```rust
pub fn rebuild(&self, text: &str) {
    ...
    *self.markdown.borrow_mut() = text.to_string();   // re-clone
}
```

**Fix:** have `rebuild` accept `String` and move it into the cell, or
inline the rebuild path in `set_markdown_prop` so the `String` argument
flows directly to storage. Saves one allocation per `set_markdown`.

Clippy's `needless_pass_by_value` flags this directly.

### 2.4 `emit_link_activated` does `uri.to_string()` — **Low**
**Location:** `src/lib.rs:84–86`.

```rust
self.emit_by_name::<bool>("link-activated", &[&uri.to_string()])
```

`&str` already implements `glib::ToValue`; `&uri` works without the
`.to_string()` allocation. Hot-ish path (per click).

### 2.5 Magic numbers in the renderer — **Low**

Carried over from the previous review's §6.4: `column_spacing(4)`,
`row_spacing(2)`, `max_children_per_line(1000)`, frame margins of `2`
/ `6` / `8`. None are named or commented; intent has to be inferred.
A line of named constants near the top of `render.rs` and a one-liner
on `column_spacing(4)` ("approximates an inter-word space at 11pt sans")
would help.

---

## 3. API design

### 3.1 No way to observe in-flight image loads — **Low**

Apps using a long-lived View as a markdown preview have no way to know
"are images still loading?" or to wait for full render. Worth a
`notify::loading` boolean property or a `loaded` signal. The generation
counter from §1.4 already gives us cancellation semantics; this builds
on it.

### 3.2 No `Display` / `From<&str>` / `From<String>` for `MarkdownTextView` — **Low**

A `From<String>` (or `From<&str>`) constructor could replace the common
`view = MarkdownTextView::new(); view.set_markdown(s);` pair. Pure
ergonomics; not blocking anything.

### 3.3 `parser` module is `pub` but `InlineStyle` is `pub(crate)` — **Low**
**Location:** `src/parser.rs:51–56`.

`InlineStyle` is the parser-side type that says "this content was a
heading / quote / paragraph". It's deliberately kept crate-internal
because it's a renderer concern; OK as-is. Worth a short comment
explaining *why* it's not part of the public AST so the next reader
doesn't reflexively flip the visibility.

---

## 4. Tests

### 4.1 Render tests are mostly micro-tests — **Low**

The `cfg(test) mod tests` in `render.rs` covers `heading_css_class`,
`styled_text_markup`, and `combine_emphasis`. The single `#[ignore]`d
end-to-end test asserts the *type* of each child but not its content
(e.g., that a paragraph containing a link actually has the activate-link
trampoline wired). Worth adding a test that `set_markdown("[x](u)")`
produces a Label whose markup contains `<a href="u">`.

### 4.2 `#[ignore]` lacks a reason — **Low**
**Location:** `src/render.rs:436`.

Clippy's `ignore_without_reason` flags it. Stylistically harmless;
`#[ignore = "needs a display; opt in via cargo test -- --ignored"]`
makes the constraint visible at the test site.

### 4.3 No test for the `link-activated` signal — **Low**

End-to-end tests need a display already (see §4.1); add an assertion
that the signal fires when a link's `activate-link` is triggered and
that returning `true` inhibits the default handler.

---

## 5. Project hygiene

### 5.1 No CI configuration — **Low**

No `.github/workflows/`. For a soon-to-be-published crate, even a
minimal `cargo fmt --check && cargo clippy --all-targets -- -D warnings
&& cargo test` workflow would pin quality.

### 5.2 No `CHANGELOG.md` — **Low**

The git log carries the per-section work, but a
[Keep-a-Changelog](https://keepachangelog.com/) `CHANGELOG.md` is the
expected first-stop document for downstream users tracking 0.1 → 0.2
behaviour shifts.

### 5.3 Example doesn't showcase `set_base_path` or `connect_link_activated` — **Low**

`examples/window.rs` builds an absolute path via `CARGO_MANIFEST_DIR`,
so `set_base_path` is never exercised. A second markdown source loaded
from disk with a known base path, plus a `connect_link_activated`
handler that prints the URI to stdout, would document both APIs by
example.

### 5.4 `examples/window.rs` embeds a build-time absolute path — **Low**

`format!("{}/examples/sample.svg", env!("CARGO_MANIFEST_DIR"))` bakes
the developer's repo path into the example binary. That's only ever
runnable from the same machine that built it. For an example that's
also a smoke test, fine; for a published demo, prefer a `gresource` or
the cwd-relative form.

---

## 6. Polish & lints

`cargo clippy -- -W clippy::pedantic` reports 8 warnings:

| Lint | Site | Action |
|---|---|---|
| `needless_pass_by_value` | `imp.rs:55` (`set_markdown_prop`) | See §2.3. |
| `too_many_lines` | `parser.rs:66` (`markdown_blocks`, 110 lines) | Extract per-block detectors, or `#[allow]` with a one-line rationale. |
| `cast_possible_truncation` | `render.rs:59` (`offset as u32`) | `u32::try_from(offset).unwrap_or(u32::MAX)`, or pin item count to a u32-safe bound. |
| `match_same_arms` | `render.rs:216–217` (`combine_emphasis`) | Merge the two `BoldItalic` arms: `(BoldItalic, _) \| (_, BoldItalic) \| (Bold, Italic) \| (Italic, Bold)`. |
| `elidable_lifetime_names` | `render.rs:313` (`style_markup<'a>`) | Drop the `'a` annotation; lifetime is elidable. |
| `missing_panics_doc` | `lib.rs:73` (`connect_link_activated`) | Add `/// # Panics` paragraph describing the `expect` calls. |
| `ignore_without_reason` | `render.rs:436` | `#[ignore = "..."]`. |
| `uninlined_format_args` | `parser.rs:782` (test panic message) | `panic!("expected only Text segments, got {seg:?}")`. |

None block CI. Worth one consolidated "polish" pass when convenient.

---

## 7. Recommended priority order

1. **§1.2 — Custom accumulator for `link-activated`.** Pin first-true-wins
   semantics before someone connects a second handler.
2. **§2.1 — Add inter-block spacing / margin around hr.** Cosmetic but
   cheap and obviously broken without it.
3. **§5.1 + §5.2 — CI + CHANGELOG.** Pre-publish polish.
4. **§1.3 — Real CommonMark delimiter-run pass.** Largest correctness
   gap left in inline parsing; non-trivial.
5. **Everything else** — micro-perf, missing CommonMark features
   (§1.5–§1.8), pedantic lints (§6).

---

## 8. What works well

- **Public AST.** `parser::{MarkdownBlock, InlineSegment, Emphasis}` plus
  `markdown_blocks` / `parse_inline_segments` are doc-commented,
  `#[must_use]`, and stable enough that downstream renderers can reuse
  them without forking the crate.
- **GObject properties.** `markdown` and `heading-level-offset` round-
  trip through `bind_property` / `notify::*` / GtkBuilder, with custom
  setters that early-return on no-op writes.
- **Inline runs collapse into one Pango Label.** Pure-prose paragraphs
  render as a single widget; `<a>`, `<i>`, `<b>` are Pango markup
  inside that Label, so wrapping, hyphenation and selection are Pango's
  problem rather than FlowBox's.
- **Async image loading.** `gio::File::read_future` +
  `gdk_pixbuf::Pixbuf::from_stream_future` keeps disk I/O and decode off
  the main thread; the synchronous `is_file()` check is fast enough to
  keep the placeholder fallback flicker-free.
- **Image size cap.** A `OnceLock`-installed CSS provider applies
  `max-height: 480px` to every Markdown picture, so a 4000×3000 PNG
  can't blow up the parent ScrolledWindow.
- **`link-activated` signal.** Apps can intercept link clicks for
  in-app routing without subclassing the View.
- **Pango-markup escaping is consistent.** Every `format!` that builds
  markup goes through `escape_markup` (now a `Cow<'_, str>` to skip the
  allocation when no escaping is needed).
- **Tests are tight.** 33 fast parser/renderer-helper tests + one
  ignored end-to-end test that opts in to a display. Edge cases (CRLF,
  unmatched `**`, empty input, unclosed code fence) are pinned.
- **No `unwrap`/`expect`/`panic!` on input paths.** Malformed input
  emits plain text; missing files fall back to an italic
  `[image: alt]` placeholder.
- **Clean module split.** `parser` produces tokens, `render` consumes
  them, `imp` glues to GObject, `lib` is the public surface. Each
  module owns its own tests.

---

*Generated against `main` after the 18-commit polish pass.*
