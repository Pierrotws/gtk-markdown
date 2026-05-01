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

- `cargo test` — 28/28 pass.
- `cargo clippy --all-targets` — clean at the default lint level.
- `cargo clippy -- -W clippy::pedantic` — 5 warnings (cosmetic; itemized in
  §6.4).
- `cargo doc --no-deps` — builds without warnings.
- `cargo build --example window` — clean.

---

## 3. API design

### 3.4 No way to override link click behaviour — **Low**

Links rely on `gtk::Label`'s default `activate-link` handler, which goes
through `gio::AppInfo::launch_default_for_uri`. Apps embedding this widget
inside a wiki-like environment can't intercept clicks (e.g., to navigate
in-app for `app://...` URIs).

**Fix:** expose a public `connect_link_activated` style signal (or wrap
each link's `activate-link` and forward).

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

1. **§5.3 — Fill obvious test gaps** (end-of-input edge cases) — pure
   addition, near-zero risk.
2. Everything else (polish, micro-perf, optional features).

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
