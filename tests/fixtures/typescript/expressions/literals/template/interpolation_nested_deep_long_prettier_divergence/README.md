# interpolation_nested_deep_long_prettier_divergence

An interpolation with **no** newline in `${…}` stays on one line in both
formatters even past printWidth (atomized) — a long line is fine. This fixture is
the other half: a **multi-line-source** interpolation, deeply nested (a template
whose interpolation `.map()`s to another template) so its inner interpolations sit
at a large visual indent.

There, a member chain / ternary whose source spans lines overflows printWidth at
that nested position. tsv breaks it to keep the line ≤100; Prettier leaves it on
one **101–109-char** line — its `addAlignmentToDoc` reset measures the overflow as
"fitting."

The divergence is **authoring-triggered**: `variant_no_newline.svelte` is the same
document with every `${…}` on one line (no newline). There both formatters keep it
inline past printWidth — tsv preserves it exactly, matching `output_prettier.svelte`.
tsv only wraps when the source spans lines; a compact `${…}` is dual-stable.

- e/f: `${ssss⏎.aaa()⏎.bbbb()⏎.ccc()}` — tsv breaks the chain; Prettier inlines it at 101–108 chars
- g2/g7/g8: `${⏎ cond ? a : b ⏎}` — tsv breaks the ternary; Prettier inlines it at 101–109 chars

This is Prettier being inconsistent with itself: at **normal** nesting it breaks
the same overflowing interpolation (agreeing with tsv — see
[interpolation_newline_positions](../interpolation_newline_positions/)); only the
deep-nesting reset makes it collapse here. tsv wraps consistently at every depth.

Compact interpolations (no newline) atomize inline in both — see
[interpolation_boundary](../interpolation_boundary/) and
[interpolation_expression_inline_long](../interpolation_expression_inline_long/).
Single-level indent alignment matches Prettier — see the non-divergence
[interpolation_multiline_indent_long](../interpolation_multiline_indent_long/).
The convergent sibling cases here are boundary context: `d` / `g1` (at 100 chars)
stay inline in both; the `g4`/`g5` line comments force a break in both.

## Reason

Print width. An interpolation whose source spans lines may break; when its
collapsed form overflows, tsv breaks it to respect printWidth at the true nested
position, whereas Prettier's alignment reset lets a 101–109-char line render
inline. (Overflow on one line is fine only when the source has **no** newline —
there both formatters atomize.)

See [conformance_prettier.md](../../../../../../../docs/conformance_prettier.md) §TypeScript: Template Literals.
