# test_call_arg_comment_prettier_divergence

A test call (`it` / `test` / `describe` and friends) prints its arguments on one
flat line, however long they get. That layout has no argument-gap comment
emitter, so a comment in the leading gap (`it(⏎// c⏎'text', …)`) or an
inter-argument gap has nowhere to go. tsv gives up the flat layout for such a
call: it expands like any other call and every comment keeps the line the author
wrote it on. Prettier keeps the flat layout and relocates the comment onto the
`(` line (leading gap) or past the previous argument (inter-argument gap).

tsv: the call expands, each comment stays on its own line
Prettier: the flat layout is kept and the comment is relocated onto a code line

```
// tsv                          // prettier
it(                             it(// c
	// c                          'text', () => {
	'text',                         a();
	() => {                       });
		a();
	}
);
```

## Reason

Two independent reasons, either sufficient:

1. **Content loss.** The flat layout emits only the argument docs joined with
   `", "` plus a trailing-comment suffix after the last argument — the
   hazard-4 shape from
   [comments.md](../../../../../../docs/comments.md) (an alternate-layout
   container builder that never runs a gap lookup). Every leading and
   inter-argument gap comment was **dropped**. A layout that cannot print a
   comment must not be selected for a document that has one.
2. **Comment position.** Prettier's relocated forms move a comment across a
   syntactic boundary onto a code line, and its own second pass then keeps
   drifting: an inter-argument comment migrates **into the callback body**
   (`it('text', () => { // c`) on pass 2 and onto its own line inside that body
   on pass 3 — the comment's association changes from "leads the callback
   argument" to "first statement of the callback". `audit_signature.txt` pins the
   chain. tsv's expanded form is a one-pass fixed point.

A **glued** block comment is owned by the argument it precedes and rides inside
that argument's doc, so the flat layout can print it — that case keeps the flat
layout and matches prettier (a case in this fixture). Likewise a comment
trailing the whole call, which the flat layout emits itself. The gate is
therefore keyed on comments *to emit* in the gaps, the axis that answers
"would this comment be dropped here?".

A format-ignore directive alone on its line in a gap freezes the following
argument here as everywhere else (Rule A) — see
[args_prettier_ignore_member](../args_prettier_ignore_member/).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
