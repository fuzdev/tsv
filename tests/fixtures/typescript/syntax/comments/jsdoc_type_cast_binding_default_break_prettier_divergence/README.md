# jsdoc_type_cast_binding_default_break_prettier_divergence

A JSDoc cast in a **binding default** — the `=` of an object-pattern property, an
array-pattern element, or a parameter, one value gap with one emitter — whose comment sits
mid-line (`a2 = ` precedes it) with the `(` authored on the next line.

**tsv** treats that break as unforced and reflows the `(` back onto the comment's line, so
the cast lands on the layout every wide cast already takes: comment glued to `(`, the inner
expression inside the preserved parens. Same answer as at every other value position.

**Prettier keeps the break where the author put it.** That is what separates these sites
from the declarator / assignment / class-property / arrow-body ones of the sibling
[`jsdoc_type_cast_value_gap_break`](../jsdoc_type_cast_value_gap_break_prettier_divergence/),
where prettier instead breaks after the `=` and hangs the comment on its own line above the
`(`. The difference matters for what the fixture can pin: a comment prettier **hangs** owns
its line, so tsv hangs it too and prettier's form is dual-stable (`variant_hung` there),
while a comment prettier leaves **mid-line** is still one tsv reflows — so prettier's form
here is a `prettier_variant_break`, which is why these sites need a fixture of their own.

`input.svelte` is byte-identical in both formatters; the divergence rides
`unformatted_ours_break.svelte`, which tsv normalizes back to input and prettier carries to
`prettier_variant_break.svelte`.

Two of the four cases are controls. `a1` is short enough that the parens stay flat, where
both formatters agree from either authoring. `a3`/`a4` are why the **array** default is in
the fixture without being a divergence: prettier collapses the run back onto the element's
line even while the pattern is BROKEN, because the array family wraps each element's
leading run plus the element in a group of its own, so the soft `line` is measured against
that element alone ([comments.md](../../../../../../docs/comments.md) §Array family vs
params family) — where the object pattern and the parameter list leave it to the broken
enclosing group and it breaks. One authoring, three binding-default sites, two prettier
answers.

The enum member's `=` is the fourth site this gap's rule reaches, and it is TypeScript-only,
where prettier's cast-unaware parser strips the parens as well — so it carries a second,
already-cataloged divergence and gets its own fixture:
[`jsdoc_type_cast_enum_member_break`](../jsdoc_type_cast_enum_member_break_prettier_divergence/).

## Reason

**Design choice.** The break is unforced — a block comment does not run to end-of-line, so
nothing pushes the `(` off the comment's line — and tsv reflows an unforced break at every
value position (see
[conformance_prettier.md §Authored breaks in value position](../../../../../../docs/conformance_prettier.md#authored-breaks-in-value-position),
which names parameter defaults among them). The cast is not an exception to that rule:
reflowing puts it on the same comment-glued-to-`(` layout the wide-cast fixtures already pin.

Where tsv and prettier part is only what happens to the break, which is the standing
[§Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
difference in its value-position form: prettier preserves the authored line break here and
tsv reflows it, exactly as it does for a plain block comment in the same gap.

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
