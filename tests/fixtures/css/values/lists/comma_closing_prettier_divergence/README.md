# comma_closing_prettier_divergence

A comma **closing** a declaration's value (`transition: a,`, `--x: a,`,
`rgb(1, 2, 3,)`) is a separator with no entry after it. tsv keeps it; prettier
deletes it.

tsv: `transition: a,` · `linear-gradient(red, blue,)` · `--x: a,`
Prettier: `transition: a` · `linear-gradient(red, blue)` · `--x: a`

The rule is **token preservation**: a declaration value is text tsv re-spells, and a
comma the author wrote is one of its tokens. Deleting it leaves the comma-split parse
untouched — CSS Syntax 3 §"parse a comma-separated list of component values" stops once
the input is empty, so `a,` and `a` are the same one-entry list — but that is the answer
to a question a declaration never asks. A declaration is matched, not split.

What the token *does* is what makes the deletion a rewrite rather than a tidy, and it
differs by declaration kind.

A **custom property** settles it with no grammar involved. Its value is a verbatim token
sequence (css-variables-1 §"Custom Property Value Syntax") that `var()` substitutes
textually, so the comma is not punctuation *around* the value — it is *part of* it.
`f(var(--x) b)` is `f(a, b)` when `--x` is `a,` and `f(a b)` when it is `a`. There is no
reading on which the two declarations say the same thing. WPT's
`css-variables/variable-declaration-21` is built on exactly that shape (`--b: 0,;
--c: 128,;` feeding `rgb(var(--b)var(--c)var(--d))`).

A **grammar-bearing** declaration is matched against its own production, and css-values-4
§"Component value combinators" requires a comma to be omitted when "all items following
the comma have been omitted" — so `transition: a,` does not match and `transition: a`
does. Note precisely what that establishes: the two forms **mean different things** (the
UA drops one and applies the other), so deleting the comma changes the page. It does not,
by itself, oblige a formatter to keep it — a formatter is free to repair invalid CSS.
That obligation is tsv's, and it is the same one
[comma_trailing_empty_element](../comma_trailing_empty_element_prettier_divergence/)
already takes one comma later in the same list: re-spell the declaration faithfully
rather than silently turning a dead one live.

One rule covers both, and it is what tsv already did everywhere else in a value:
`var(--a,)` keeps its empty fallback, an opaque `url(a,)` keeps its comma, and the
comment-bearing path (which re-emits the value from source) already kept
`--x: a /* c */,`. Only the comment-free, non-`var`, non-`url` path deleted it.

## Scope

The `<media-query-list>` is the one place tsv *does* delete a closing comma, and it is a
real spec difference rather than a carve-out: mediaqueries-4 §Syntax defines the
production *as* "a comma-separated list of component values", so there the split **is**
the grammar and `@media screen,` really is the same query list as `@media screen` — see
[media_query_closing_comma](../../../at_rules/media_query_closing_comma_prettier_divergence/).

An **escaped** comma is content, not a separator, so it closes nothing and
nothing is appended after it — pinned by
[escaped_whitespace](../../escaped_whitespace_prettier_divergence/) (`gap: a, b\,`,
which prettier corrupts) and by
[var_comma_fallback](../../variables/var_comma_fallback/) (`var(--b, x\,)`, where
the two agree).

## Reason

**Content preservation.** Not spec precedence — tsv is not emitting a canonical
serialized form, it is emitting the authored one. Not ◆prettier_bug either: prettier is
idempotent here and its output is valid CSS; a meaning change on its own is content
preservation. See
[conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values)
("Closing comma in a value").

## Related

- [comma_trailing_empty_element](../comma_trailing_empty_element_prettier_divergence/) — one comma further, where the last element is empty and the list cannot be spelled without it
- [comma_empty_element](../comma_empty_element/) — a leading or interior empty element, where tsv and prettier agree
- [var_comma_fallback](../../variables/var_comma_fallback/) — `var()`'s fallback comma, which prettier keeps too
- [media_query_closing_comma](../../../at_rules/media_query_closing_comma_prettier_divergence/) — the one construct whose closing comma tsv deletes
