# jsdoc_type_cast_value_gap_break_prettier_divergence

A JSDoc cast in **value position** whose comment sits mid-line — something precedes it on
its line (`const a2 = `) — with the `(` authored on the next line.

**tsv** treats that break as unforced and reflows the `(` back onto the comment's line, so
the cast lands on the layout every wide cast already takes: comment glued to `(`, the inner
expression expanded inside the preserved parens.

```
const a2 = /** @type {A} */ (
	someObject.somePropertyName.someOtherProperty.andAnotherOne.andYetMore.final
);
```

**Prettier** hangs instead — it breaks after the `=` and puts the comment on its own line
above the `(`.

So `input.svelte` itself is byte-identical in both formatters; the divergence shows only on
the authored-break variant (`unformatted_ours_break.svelte`), which tsv normalizes back to
input and prettier carries to its hang form. That hang form is pinned as
`variant_hung.svelte` and is **dual-stable**: once the comment has a line of its own,
`jsdoc_cast_comment_is_own_line` is true, so tsv hangs the value and keeps it exactly where
prettier does. Pinning it makes the round trip explicit — the mid-line authoring is the only
unstable one, and the two formatters carry it to two different stable places.

The fixture covers a declarator, an assignment expression, a class-property initializer,
and an arrow body. `a1` is the control — short enough that the parens stay flat, where both
formatters agree from either authoring.

An **object-literal value** is deliberately not among them, even though it is a value gap
and takes the same reflow. Its `:`→value gap has a second rule of its own: the gap hangs
whenever a block comment in it is followed by a newline, and that gate reads the source
rather than the cast, so the break authoring lands on `key:⏎\t/** @type {A} */ (…)` — the
comment glued to its `(` as everywhere else, but under a hang the glued authoring never
produces. The two authorings therefore reach two fixed points there, and prettier collapses
the hang from either, so no variant marker describes the pair. That hang predates this rule
and is unrelated to the cast; what the cast contributes — the comment staying with its `(` —
is the same here as at the four sites above.

The complementary case is a cast in a position that is **not** a value gap — a statement, a
label, a call argument — where the author's break after the `*/` is kept rather than
reflowed, because those lists keep lines. That is
[`jsdoc_type_cast_leading_run_break`](../jsdoc_type_cast_leading_run_break/), and the two
fixtures together are the whole of the cast's separator rule.

## Reason

**Design choice.** The break is unforced — a block comment does not run to end-of-line, so
nothing pushes the `(` off the comment's line — and tsv reflows an unforced break at every
value position (see
[conformance_prettier.md §Authored breaks in value position](../../../../../../docs/conformance_prettier.md#authored-breaks-in-value-position)).
The cast is not an exception to that rule: reflowing puts it on the same
comment-glued-to-`(` layout the wide-cast fixtures already pin
([jsdoc_cast_paren_span](../../../declarations/variable/jsdoc_cast_paren_span/),
[jsdoc_cast_call_arg_long](../../../expressions/calls/jsdoc_cast_call_arg_long/)), so one
authoring difference does not produce two different layouts for the same cast.

Where tsv and prettier part is only what happens to the break, which is the standing
[§Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
difference in its value-position form: prettier preserves the authored line break here and
tsv reflows it, exactly as it does for a plain block comment in the same gap.

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
