# Computed key, own-line block comment in the `[`→key gap

A computed key never breaks on **width** — both formatters keep a long key inline — so the
only thing that opens the brackets is a comment. This fixture pins which comments do:
a block the author **glued** to the key leads it inline and the brackets stay flat
(`[/* c */ bbb + ccc]`), while a block the author gave its **own line** keeps that line and
the brackets open around it. Both authorings are fixed points; the placement is the author's.

Prettier opens the brackets too, but writes its own placement: the comment is glued flush to
the `[` and the `]` welds onto the key's line (`[/* c */⏎fff + ggg]: hhh`).

## Why tsv differs

Own-line-ness is a question about the comment's own neighbours, and a comment the author put
on its own line carries that as authorship — collapsing it onto the key's line reflows a
break the author wrote, and gluing it to the `[` is the open-delimiter relocation tsv
declines everywhere else. Keying the separator on the
comment's KIND alone (every block a space) rather than on what surrounds it collapses it — the
hand-rolled split `docs/comments.md` §Leading comments exists to prevent, and the same one
the expression-side template interpolation carried.

The collapse also silently disarmed a **block-spelled** format-ignore directive in this gap:
the placement floor reads only a directive alone on its line, so pulling it onto the key's
line made it inert while the `//` spelling froze. Preserving the line makes the two spellings
agree — see
[computed_key_prettier_ignore_head](../computed_key_prettier_ignore_head_prettier_divergence/).

## Expected behavior

- **tsv**: each authoring is preserved and is its own fixed point, at object properties,
  class members and destructuring patterns alike; a run the author broke keeps every line.
- **prettier**: opens the brackets but glues the comment to `[` and welds the `]` onto the
  key's line (`output_prettier.svelte`), and is not idempotent on its own output —
  pinned by `audit_signature.txt`.

## Reason

◆comment_preservation. See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
("Object/array/block open-delimiter trailing", which covers the `[`→key gap), the
format-ignore consequence in
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(under *Computed key*), and the governing principle in
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
The glued/`//`-spelled faces of the same gap are
[computed_key_glued_block_comment](../computed_key_glued_block_comment_prettier_divergence/)
and
[computed_key_open_bracket_line_comment](../computed_key_open_bracket_line_comment_prettier_divergence/).
