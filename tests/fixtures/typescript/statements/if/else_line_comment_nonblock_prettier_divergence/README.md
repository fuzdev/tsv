# else_line_comment_nonblock_prettier_divergence

Line comment between the `else` keyword and a non-block body expression.

Prettier 3.9 keeps the comment after `else` but drops it onto its **own line**,
indented with the body:
```
} else
	// c1
	expr;
```

**tsv**: keeps the comment trailing `else` on the **same line** (user placement),
body indented:
```
} else // c1
	expr;
```

(Under prettier 3.8 the comment was relocated *before* `else` — `} // c1\nelse
expr;`. Prettier 3.9 no longer relocates it there, but that form is still
prettier-stable, documented as the dual-stable `variant_comment_before_else.svelte`.)

Per Comment Position Philosophy: preserve user intent (the comment trailing the
`else` keyword) rather than forcing it onto its own line.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
