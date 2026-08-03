# ws_sensitive_welded_dangle_long_prettier_divergence

Inside `<pre>`, an over-width welded run (`…<b>welded</b>tail …`, 101 cols) has no
whitespace boundary to break at — every inter-tag byte is content.

tsv: leaves the over-width line intact (print width yields to render semantics)
Prettier: dangles the closing tag's `>` (`</b⏎\t>tail`) to duck under printWidth

## Reason

**Design choice — a deliberate weighing, not an impossibility.** A render-free break
*does* exist here: between `</b` and `>` is tag syntax, and prettier takes it. tsv
declines it because the only available break is the tag-delimiter dangling it excludes
everywhere else (see `inline_closing_intact_long` and §Svelte: Inline content
block-style — tsv lays content out with both tags intact, prettier dangles delimiters),
and adopting the mechanism solely inside `<pre>` costs more than the width violation it
would cure:

- A continuation line led by `>` reads as content — and `<pre>` content is
  disproportionately code samples where a literal `>` is ordinary, so the dangled form
  is at its most confusing exactly where it would appear. It also breaks
  source-mirrors-render, the point of `<pre>` authoring.
- The machinery would insert breaks inside tag syntax within verbatim subtrees, where
  a one-byte miss is silent content corruption — and prettier's own machinery in this
  region carries a cataloged bug (`ws_sensitive_attr_comment_line`: comment ejection,
  non-idempotent), so exact conformance is not even the target.
- Prettier's behavior here is the emergent output of its generic dangle-everywhere
  inline printer (which tsv rejected wholesale), and its pack/dangle decisions are
  shape-sensitive; tsv would be building dedicated machinery to chase a side effect.
- The shape occurs in no known real code (zero corpus hits) — the sanction is free in
  practice, and cheap to reverse: tsv already parses and rejoins the dangled authoring,
  so flipping later is an implementation plus a fixture conversion, no migration.

Both formatters agree the *content* never re-wraps, and both keep the 100-col case
intact — the divergence is only the tag-syntax dangle past printWidth. tsv additionally
normalizes an authored dangle back to the intact line (`unformatted_ours_dangled` —
tag-syntax whitespace is not content, so the rejoin is render-free and the document has
one fixed point); prettier keeps the dangled authoring stable.

The two cases pin the **100/101 boundary**: at exactly 100 both formatters leave the run
intact; one char over, prettier dangles and tsv stands.

See [conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy).
