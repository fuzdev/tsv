# css_boundary_run_line_separator_prettier_divergence

A `prettier-ignore`d CSS rule whose boundary run is a U+2028 LINE SEPARATOR (`<LS>`) or
U+2029 PARAGRAPH SEPARATOR (`<PS>`). Both are JavaScript line terminators, so `parseCss`
steps over them as boundary whitespace and the rule starts at `a` / `b`. css-syntax-3 reads
them differently: its whitespace is `<LF>`, tab and space only, so to a browser each is a
`<delim-token>` in front of the selector. Dropping the character would change the rule the
browser sees.

Both formatters keep the frozen slice from the run's first character, so the rule prints
exactly as written, the same as every other non-ASCII run in front of a frozen node
([css_boundary_run](../css_boundary_run/)).

The divergence is the line **above** the run. Prettier's `isNextLineEmpty` counts `<LS>` /
`<PS>` as line terminators, so the one `<LF>` after the directive plus the `<LS>` reads as a
blank line, and `output_prettier.svelte` opens one after each directive. tsv's blank-line
rule stops at `<LF>` / `<CR>` and prints no blank line.

## Reason

tsv keeps these characters in its output and regenerates the newline beside them, so a
class that counted them would read the author's single terminator plus tsv's own newline as
a blank line the author never wrote, and it would appear on the second pass. See
[conformance_prettier_css.md §CSS: Comments](../../../../../../docs/conformance_prettier_css.md#css-comments)
(the "Blank line across a `<LS>` / `<PS>`" entry) and
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
