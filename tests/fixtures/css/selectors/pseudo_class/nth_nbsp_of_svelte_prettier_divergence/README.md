# nth_nbsp_of_svelte_prettier_divergence

A non-ASCII space (U+00A0) at the `of` juncture of `:nth-child(An+B of S)`: after the
keyword (`of<NBSP>.class1`), before it (`2n + 1<NBSP> of`), and after `S` before the `)`.
`parseCss` skips each one as whitespace — its `REGEX_NTH_OF` terminator is `\s+of\s+`, a JS
regex, and its `allow_whitespace()` is the same `\s` — so the character is a separator to the
parser and content to the output: tsv keeps it where the author wrote it, as prettier does.

## Svelte divergence

The same nesting divergence as
[nth_child_of](../nth_child_of_svelte_prettier_divergence/): Svelte folds the ` of ` and its
surrounding whitespace into `Nth.value` (`"2n + 1 of<NBSP>"`) and reads `S` as sibling
selectors; tsv keeps `Nth.value = "2n + 1"` with `S` nested under `Nth.selector`. The run
falls in the gap between the two, so nothing new reaches the wire — only the position where
the run stops being part of a value. The `:is()` case matches Svelte (the fold is kept there,
run included) and is in both ASTs alike.

See [conformance_svelte.md §CSS Corrections](../../../../../../docs/conformance_svelte.md#css-corrections).

## Prettier divergence

tsv always emits single spaces around `of`; prettier collapses whitespace runs there but
never inserts an absent space (the `nth_child_of` rule). A `<NBSP>` glued to the term
(`2n + 1<NBSP>of`) is therefore prettier-stable — `prettier_variant_of_glued` pins it — where
tsv puts the keyword's space after the run (`2n + 1<NBSP> of`). After the keyword the two
agree: the run stands in for the space (`of<NBSP>.class1`), since an ASCII space beside it
would be a second separator.

See [conformance_prettier_css.md §CSS: Selectors](../../../../../../docs/conformance_prettier_css.md#css-selectors).
