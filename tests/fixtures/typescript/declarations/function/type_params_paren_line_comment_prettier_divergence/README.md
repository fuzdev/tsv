# type_params_paren_line_comment_prettier_divergence

A line comment between a type-parameter list's `>` and the parameter list's `(`
(`<T> // c⏎(p: T)`) runs to end of line, so the `(` cannot stay on it. tsv keeps
the comment where the author wrote it and drops the parameter list to a
continuation line **indented one level** — the uniform forced-continuation
indent. Prettier instead relocates the comment out of the gap, and answers the
same gap two different ways depending on the spelling.

```
// tsv                          // prettier
const a = <T extends A> // c1   const a = <T extends A>( // c1
	(p: T) => p;                  p: T
                                ) => p;

function fn1<T> // c2           function fn1<T>(p: T) { // c2
	(p: T) {                      return p;
	return p;                   }
}
```

For the arrow the comment lands **inside the parens** as a leading comment on the
first parameter; for a `function` declaration, a function expression and a class
method it lands **after the body's `{`** — and prettier is not idempotent there,
a second pass moving it again onto its own line inside the body (pinned by
`audit_signature.txt`). tsv gives all four spellings one answer.

The same gap's **block** comment is already preserved in place by tsv in every
spelling, so this makes the line comment agree with its own sibling rather than
introducing a new rule.

## Reason

Relocating out of this gap is **information-destructive on a run**, which is what
decides it. Two stacked line comments merge onto one line — `const c = <T extends
A>(p: T) => p; // c5 // c6` — where the second `//` becomes text inside the first
and stops being a comment at all; and a block written *after* a line comment
(c7/c8) **reorders** ahead of it, the inline block jumping the deferred one. tsv
keeps every comment distinct, in source order, on the line the author gave it.

Per [conformance_prettier.md §Comment Position
Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
the deciding test is information loss, not position purity — and this gap is not
the sanctioned pure-separator trail: the `>`→`(` boundary is a construct
boundary, not a separator, and a deferred run must not leave the construct it was
written in.

The continuation **indent** is tsv's own layout choice, applied wherever a line
comment splits a construct's head from its tail, so the tail reads as part of its
construct rather than as a sibling statement. See [conformance_prettier.md
§Uniform Forced-Continuation
Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent).

A **block** comment forces nothing (`function fn2<T> /* c9 */(p: T)` stays
inline, matching prettier) — it is pinned here as the control for the
line-comment rule. The arrow's block spelling carries a second, separate
divergence and is pinned in
[arrow_type_params_paren_comment](../arrow_type_params_paren_comment_prettier_divergence/).

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
