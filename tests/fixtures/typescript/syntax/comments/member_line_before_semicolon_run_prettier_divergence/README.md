# member_line_before_semicolon_run_prettier_divergence

A line comment written between a member's last token and its `;` trails **after**
the `;` in both formatters — that much is the non-divergence
[`line_before_semicolon`](../line_before_semicolon/). The divergence starts with
whatever the author put on the `;`'s own line behind it.

Because the member's doc now ends with that deferred comment, nothing may share
its output line. tsv hands the second comment to the next member's leading run, so
it takes the line below — both comments distinct, both in source order. Prettier
appends it to the same line instead: two line comments **merge** into one
(`a = 1; // c1 // c2`, the second `//` becoming text of the first, so a comment
stops existing), and a block is **reordered** ahead of the comment it was written
after (`a = 1; /* c2 */ // c1`).

`input.svelte` is dual-stable — both formatters keep it — so the split rides
`unformatted_ours_semi_own_line.svelte`, the same members authored with the `;` on
its own line: tsv reflows it to `input.svelte`, prettier merges and reorders.

Prettier's output from that variant is pinned as `variant_merged.svelte`, and it is
**dual-stable**: once `// c2` has become text inside `// c1` there is no comment left
to split back out, so both formatters keep the merged form verbatim. That is the
whole argument for the divergence — prettier's fixed point here is one comment short
of the input, and the loss is not recoverable by reformatting.

What the member deferred may equally be an own-line **block** rather than the
trailing `//` (`a: A // c1⏎/* c2 */; /* c3 */`, the `T3` case). The rule does not
change — the block takes the line below and the trailing comment follows it there —
and neither does prettier's answer, which still hoists the trailing comment onto
the `//`'s line and pushes the block beneath it.

On the **last** member of a body the rule is the same but a different emitter answers
it: there is no following member to lead the comment, so the body's own end-of-run
emitter claims it (the `C3` / `I3` / `T4` cases). Prettier merges there too.

A type member's separator may equally be a `,` — tsv normalizes either spelling to the
`;` the member prints — so the same rule holds for the comma authoring, pinned by
`unformatted_ours_comma_separator.svelte` (the class arms keep `;`, which is their only
separator). Prettier reaches the same merged fixed point from both authorings, so both
variants land on `variant_merged.svelte`.

The divergence is **member bodies only**. At statement level prettier keeps the
pair distinct too, which is what the statement and block walks already do; class,
interface and type-literal bodies all take the member rule here. The non-divergent
face of the same seam — where what trails the `;` is a `//` and all three
containers agree with prettier — is
[`member_deferred_gap_trailing_comment`](../member_deferred_gap_trailing_comment/).

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
