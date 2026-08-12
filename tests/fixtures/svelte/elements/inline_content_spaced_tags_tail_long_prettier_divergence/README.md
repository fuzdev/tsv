# inline_content_spaced_tags_tail_long_prettier_divergence

An inline element whose content run overflows, so the element lays out **block-style** — and the
run inside it then **packs as one fill**, exactly as the same run packs inside an element made
multiline by its own authored newlines.

That equality is the point. A prose-bearing run has one interior layout, and *why* its element
became multiline is not part of the question. Before, the two modes held contradictory policies:
a run in a **width-broken** element took one tag per line (the separator before a tag was a bare
`line`, which resolves all-or-nothing with the parent group — and the parent is already broken
whenever the element overflowed), while the same run in a **newline-authored** element packed
(the sibling-newline flow rule, see
[inline_adjacent_sibling_newline_flow](../inline_adjacent_sibling_newline_flow_prettier_divergence/)).
Only the reflowable-fill suppression kept those two apart, by keeping the authored form out of
the multiline arm entirely; once authored air is honored in inline containers
([inline_boundary_air](../inline_boundary_air/)) the same document reaches both modes, and two
interior policies become a two-pass cycle rather than a difference of taste.

## Cases

- **100 chars** — fits, stays inline (the control).
- **101 chars** — overflows; block-style, and the run packs on the content line.
- **element lead** — a `<Comp />` heading the run, with the tag glued to its trailing word: the
  same packed layout, so what heads the run does not change it either.
- **authored air** — the same run in an element made multiline by its own boundary newlines
  rather than by width. This is the parity assertion the whole fixture turns on: the two
  multiline *causes* reach one interior. Prettier splits this one while dangling the width-broken
  twin above, so the pair also shows the divergence is about the interior rather than about
  either cause.

`unformatted_ours_one_per_line.svelte` is prettier's form fed back in — tsv packs it to `input`
in one pass, which is what makes the packed form the single fixed point rather than one of two.

## Prettier's form

`output_prettier.svelte` splits the run one tag per line and keeps that stable. This is the same
divergence [inline_adjacent_sibling_newline_flow](../inline_adjacent_sibling_newline_flow_prettier_divergence/)
already records — tsv converges an adjacent tag pair onto one line where prettier splits it even
though the line fits — reached here through width rather than through authoring. Every boundary
tsv collapses is inter-node whitespace that renders as one space either way, so the output
renders identically to the input.

## Reason

Design choice: one interior layout per run, independent of the element's multiline **cause**.
tsv converges the width-broken and newline-authored spellings of one document onto a single
fixed point, where prettier holds a distinct stable form for each.
See [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
