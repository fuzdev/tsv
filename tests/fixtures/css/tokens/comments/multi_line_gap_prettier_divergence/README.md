# multi_line_gap_prettier_divergence

Multi-line block comments in gap positions (selector comma gap, before `{`, declaration
value) keep their interior verbatim — continuation lines keep their authored column and
are never reindented by the `<style>` embedding, so the comment content is byte-stable
across passes.

tsv: normalizes the gap layout — the comment joins its line with single-space separation
(`.class1, /* x⏎y */ .class2 {`), interior verbatim
Prettier: preserves the authored layout verbatim (an own-line comment stays on its own line)

## Reason

Stable quirk. The same single-space gap normalization as
[selector_list](../selector_list_prettier_divergence/), extended to multi-line comments;
the divergence is only in which stable layout each formatter picks — both preserve the
comment interior byte-for-byte. See
[conformance_prettier.md §CSS: Comments](../../../../../../docs/conformance_prettier.md#css-comments).

## Related

- [selector_list](../selector_list_prettier_divergence/) — single-line comma-gap comments (same normalization)
- [multi_line](../multi_line/) — between-rules multi-line comment (no divergence)
