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

- `cargo test` — 33/33 pass (plus 1 #[ignore]d render-pipeline test
  opted in via `cargo test -- --ignored`; needs a display).
- `cargo clippy --all-targets` — clean at the default lint level.
- `cargo doc --no-deps` — builds without warnings.
- `cargo build --example window` — clean.

All Critical, High, and Medium findings are resolved. The only Low item
left is §6.4.

---

## 6. Polish & style

### 6.4 Magic numbers in the renderer — **Low**

`set_max_children_per_line(1000)`, `set_column_spacing(4)`,
`set_row_spacing(2)`, frame margins of `2` / `6` / `8`. None are
self-describing and a reader has to infer intent. A short named constant
or one-line comment for each would help. (Per the project's "comments
only when WHY is non-obvious" rule, the column-spacing-vs-Pango-spacing
choice in particular deserves a `// Approximates a space at 11pt` note.)

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
