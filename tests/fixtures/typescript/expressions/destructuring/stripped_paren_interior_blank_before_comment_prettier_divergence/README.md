# stripped_paren_interior_blank_before_comment_prettier_divergence

The blank-line face of
[stripped_paren_interior_own_line_comment](../stripped_paren_interior_own_line_comment_prettier_divergence/):
an own-line block comment inside a stripped paren shell at a binding default's
tail, with a blank line the author wrote **ahead of it** (`a = (1⏎⏎/* c */)`).

Both formatters erase the shell, so the comment lands in the list's own gap and
they hand it to different elements — tsv slides it forward to lead the next item,
prettier hoists it backward across the `=` and the binding name. Same divergence,
same reason as the sibling; this fixture adds what happens to the blank.

The blank is **authorship, not shell structure**, so tsv keeps it: it separates
the comment from the item above, and it says so in the erased form exactly as it
did in the authored one. The distinction is drawn by `Printer::element_shell_end`,
which steps over the erased `)` and whitespace but **stops at the first comment** —
so a blank with nothing but the shell behind it is erased with the shell
([stripped_paren_interior_blank](../stripped_paren_interior_blank/)), while a
blank the author put ahead of a comment survives.

**The blank needs a line to survive on, and the comment does not supply one.** Its
own line ends with the erased `)` and the comma, so it is glued and forces nothing
open — every list here flattens when it fits, whichever side of the comment the
author's line break fell
([stripped_paren_interior_own_line_comment](../stripped_paren_interior_own_line_comment_prettier_divergence/)
is that same authoring with no blank, and collapses). So both cases hold the list
open for a reason of their own — a trailing `//` on a later item — and the blank is
then preserved exactly where it was written. Collapsed, it has no position at all,
which is what an **array** pattern does with every authoring of this shape.

One rule across the object pattern and the parameter list: the comment leads the
next item inline in both (`/* c */ b1`), the glue being the author's, not the
family's. Where the author instead ends the comment's line, the two part ways —
the object pattern groups each element so the run still collapses onto it, the
parameter list does not, so it breaks. That per-family split is the existing
leading-run rule, not a property of this shape.

- `unformatted_ours_authored.svelte` — the authored shell form: tsv normalizes it
  to input, prettier to the variant.
- `variant_leading.svelte` — prettier's landing form, dual-stable: a comment
  *authored* leading an element keeps that position in both formatters, and its
  blank went with it — the gap it measured is the one the comment left.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation, and
[comments.md §The element-comma seam](../../../../../../docs/comments.md#the-element-comma-seam-the-two-runs-must-partition-the-gap)
for the claim-vs-distance anchor split this rests on.
