# close_brace_comment_prettier_divergence

Comments between an `export {...}` specifier list's closing `}` and the
terminating `;` (no `from` clause) are preserved where the user placed them.

**Prettier**: relocates them inside the specifier braces (`output_prettier.svelte`):

```
export {a as x /* c */};
export {
	b as y, // 1
	// 2
};
```

**tsv**: preserves them after `}` — a block comment trails the brace, line
comments stay on their own line with `;` following:

```
export {a as x} /* c */;
export {b as y} // 1
// 2
;
```

Per Comment Position Philosophy, the user's chosen position is preserved. Same
gap as `do_while/close_paren_comment_prettier_divergence`. Only the no-`from`
case diverges; with a `from` clause prettier keeps the comment after the source,
so tsv matches prettier there.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
