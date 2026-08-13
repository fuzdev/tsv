# function_annotated_param_comment_prettier_divergence

A comment after a callback's **last parameter** that forces the parameter list multiline (a
line comment, a multiline block, or a block on its own line) invalidates the expand-last-argument
hug: that hug renders the callee and the signature's head on one line, and a forced break inside
the signature is exactly what it cannot honor. So the call expands instead. tsv applies that
rule to every callback, and prettier applies it to a `function` expression **only while its last
parameter is a bare identifier** — the moment that parameter carries a type annotation, a
default, or is a pattern, prettier goes back to hugging.

```
// tsv                              // prettier
fn(                                 fn(function (
	function (                        y: T // c1
		y: T // c1                    ) {
	) {                                 call(y);
		call(y);                      });
	}
);
```

tsv is uniform because **prettier's split is not a layout decision** — it falls out of comment
attachment. `handle-comments.js`'s last-function-argument handler fires only when the node
preceding the comment is an `Identifier` (or a pattern / rest / parameter property), and an
annotated parameter puts the *type* node there instead, so the handler declines and the comment
lands somewhere that flips the expansion. Nothing about `y: T` versus `y` changes what the layout
can render; a type annotation deciding whether the enclosing **call** expands is an artifact, not
a rule tsv can restate. Both prettier forms are stable fixed points, so this is a divergence
rather than a convergence bug.

The argument for expanding is the one tsv's own **arrow** twin already follows in every one of
these shapes, prettier included — `c7` pins that, and it is the reason uniformity here costs
nothing in coherence: the alternative was a formatter whose arrow and `function` callbacks answer
the same question oppositely.

The bare-identifier cases, where tsv and prettier agree, are the convergent half of this rule and
live in
[function_callback_trailing_param_comment](../function_callback_trailing_param_comment/). A
**single-line** block forces nothing and hugs in both — pinned there too.

⚠️ **`new` is deliberately not on this rule.** Prettier hugs a `function` argument under `new`
uniformly, annotated or not, so there is no incoherence to correct and tsv matches it on both
callback kinds; the control is in the convergent fixture.

Covers: annotated (`c1`, `c2`), defaulted (`c3`) and destructured (`c4`) last parameters, the
multiline-block kind (`c5`), a member chain's argument (`c6`), and the converging arrow twin
(`c7`).

Reason: `◆design_choice`. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
