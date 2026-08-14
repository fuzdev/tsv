# import_open_paren_comment_prettier_divergence

A comment trailing a dynamic `import(`'s opening paren on the same line
(`import( // c` or `import( /* paren */`) is preserved on the `(` line. Prettier
relocates it to its own line as the specifier's leading comment.

```
// tsv                          // prettier
import( // c1                   import(
	'./a'                            // c1
);                                './a'
                                );
```

The **import type** (`import('./d').B`) answers the gap identically — the two
constructs share one argument layout (`build_import_args_comment_layout`), and the
type-level printer drifting from it is the failure mode that layout exists to
prevent.

## Reason

The same rule the plain call already follows
([open_paren_comment](../open_paren_comment_prettier_divergence/), and its
[chain](../chain_open_paren_comment_prettier_divergence/) /
[new](../new_open_paren_comment_prettier_divergence/) spellings): tsv treats user
comment placement as intentional, and a comment the author parked after `(` is a
trailing comment on that line. `import(…)` is a call shape and had been the lone
member of the family to relocate it — an internal inconsistency, not a decision,
since nothing pinned either import spelling.

When the author writes the comment on its own line instead, both formatters keep it
there — the two positions are dual-stable.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
