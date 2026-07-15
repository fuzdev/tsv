# condition_breaking_comment_prettier_divergence

A `{#if …}` head broken by a **comment** rather than by width. tsv indents the continuation
and dangles the closing `}` at the tag's base indent; prettier keeps `cond}` hugged.

This is the same head-wrap shape as [long_prettier_divergence](../long_prettier_divergence/) —
only the *trigger* differs. tsv's block head hugs its first fragment to `{#if`, indents what
follows, and drops `}` to base:

```
{#if a &&          {#if // c1
	b                  cond
}                  }
```

The rule is keyed on **that the head broke, not on why**, which is the whole reason this is a
sanction and not a second layout: a head can break because it exceeds printWidth or because a
comment forces it, and both produce one shape. Prettier never dangles a block head's `}`
whatever broke the head.

Boundary shapes covered:

- **line comment** — ends in a hardline, so `cond` starts a real continuation line and takes
  the indent.
- **multi-line block comment** — its newlines live *inside* its verbatim source span, which
  renders with no context indent by design (the interior stays exactly as authored). So
  `cond` rides the comment's own last line (`c2 */ cond`); there is no continuation line, and
  nothing to indent. The `}` still dangles.
- **single-line block comment** — breaks nothing. The head stays inline and both formatters
  agree, pinning that the trigger is the *break*, not a comment's mere presence.

## Reason

See [conformance_prettier.md §Svelte: Blocks](../../../../../../docs/conformance_prettier.md#svelte-blocks)
for the head-wrap + `}` dangle + body-expand model and why tsv diverges (consistent with its
JS `if (⏎…⏎) {` and broken-element `>`; block-body whitespace is render-non-significant).

Note this is **not** an instance of the print-width divergence, though it shares its shape:
the head here is far under printWidth, so prettier is not choosing to run long. It simply
does not dangle.

## Related

- [long_prettier_divergence](../long_prettier_divergence/) — the same shape, triggered by width
- [if_brace_in_regex_comment](../if_brace_in_regex_comment/) — a `}` inside a comment/regex in
  the head must not close the tag early (a parse concern, not a layout one)
