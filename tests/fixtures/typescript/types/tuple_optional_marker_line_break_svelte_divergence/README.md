# Line break before a tuple element's `?` (`[T⏎?]`) — Svelte Divergence

The postfix optional `?` of a tuple element is a `[no LineTerminator here]`
position. tsc runs its whole postfix suffix loop — `?`, `!` and `[` alike — under
`while (!scanner.hasPrecedingLineBreak())` (`parsePostfixTypeOrHigher`), so
`[T⏎?]` is not an optional element: the element ends at `T` and the stray `?`
fails with *`',' expected`*. oxc rejects it the same way.

acorn-typescript accepts it — its `tsParseTupleElementType` bare-`eat`s the `?`
while spelling the guard for the array suffix one function below
(`tsParseArrayTypeOrHigher`: `while (!hasPrecedingLineBreak() && eat('['))`).
Babel, which acorn-typescript ports, has the same asymmetry. **tsv** applies the
guard at both, matching tsc and oxc: `Optional tuple element `?` cannot follow a
line terminator`. Its array-suffix sibling is pinned by
[asi_postfix_bracket_type.rs](../../../../asi_postfix_bracket_type.rs).

The **named**-member marker is a different grammar position and does take the
break (`[a⏎?: T]` — tsc reads it through `parseOptionalToken`, outside that
loop); tsv accepts it too.

Per ecma262 §sec-comments a block comment holding a line terminator *is* one, so
the comment-borne authorings `[T // c⏎?]` and `[T /* c⏎ */?]` are rejected on the
same rule. Those, and the same-line control `[T /* c */?]` that stays an optional
element, are pinned by
[tuple_optional_marker_line_break.rs](../../../../tuple_optional_marker_line_break.rs);
the same-line comment gap's formatting lives in
[tuple_optional_comment](../tuple_optional_comment/).

Because the canonical parser accepts this input, the rejection cannot be an
`input_invalid_*` fixture (which requires both parsers to reject). This
`tsv_rejects.txt` fixture pins the divergence from the other side: tsv rejects
(`tsv_rejects.txt` substring), while `expected_svelte.json` proves acorn still
accepts.

**Upstream**: @sveltejs/acorn-typescript — `tsParseTupleElementType` omits the
`hasPrecedingLineBreak` guard on the optional `?`.

See [conformance_svelte.md](../../../../../docs/conformance_svelte.md) §TypeScript Corrections
(Line break before a tuple element's `?`).
