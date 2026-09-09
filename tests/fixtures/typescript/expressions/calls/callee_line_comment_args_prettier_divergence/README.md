# callee_line_comment_args_prettier_divergence

The **non-empty**-argument twin of
[callee_line_comment_empty_args](./../callee_line_comment_empty_args_prettier_divergence/): a
line comment in the callee→`(` gap, with arguments present. Same gap, same case structure —
so the two argument spellings can be read side by side.

tsv keeps the comment on the side of the `(` the author wrote it on, and the whole argument
list drops to a continuation line:

```ts
call // c1
	(a);
```

Before this, the two spellings answered one gap differently: the empty list kept the comment
outside (above) while a list with arguments pulled it **inside** the parens
(`call( // c1⏎\ta⏎);`), collapsing the two authorings `call // c⏎(a)` and `call( // c⏎a)` onto
one output. The rule is now the `?.` split's, one delimiter over — a comment belongs to the
side of the delimiter it was written on — and it reaches every keyword that opens an argument
list: a plain callee, `new`, a member callee, a long chain, and `import`.

## Divergences, by arm

- **The continuation indent.** Prettier keeps the continuation **flush** (`call // c1⏎(a);`)
  at every arm, which is the standing
  [§Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)
  divergence the empty-argument twin already carries; the comment does not move in either
  formatter.
- **`import` (`c8`) is where the position rule costs an oracle match.** Prettier relocates the
  comment *inside* the parens (`import(⏎\t// c8⏎\t'./g'⏎);`) because it attaches by node and
  `import` has no preceding node for the comment to trail. tsv answers the gap by position at
  all three keywords instead. See
  [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
- **The stacked run (`c4`/`c5`) stays whole.** Prettier splits it — `c4` outside the parens,
  `c5` in — so one authored run comes out on two sides of a delimiter. Position-preserving
  keeps the run together.
- **The member-chain expansion (`c6`, `c7`).** Prettier expands the enclosing chain
  (`a⏎\t.b // c6⏎\t(e);`) where tsv keeps it flat and breaks only where the `//` requires it —
  the same chain divergence the empty-argument twin records.

## Controls

- **`c3`** — the gap *after* explicit type arguments is this gap, not the callee→`<` one, so
  the split happens past `<A>` and the type arguments stay on the callee's line.
- **`c9`** — a **block** comment forces nothing and leads the first argument in both
  formatters (`call(/* c9 */ h)`), so the gap is not split for one. This is what keeps
  `import /* c */ ('m')` intact as well, and it is the boundary of the whole rule: only a `//`
  ends its line, and only a run that ends its line has a side to preserve.

## Reason

One gap, one answer, at every argument spelling and every keyword. Inlining a `//` here was
content loss in the empty-argument case (the comment swallowed the call's own parens and the
`;`); with arguments present it is position loss instead, which
[§Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
answers the same way.

## Related

- [callee_line_comment_empty_args](./../callee_line_comment_empty_args_prettier_divergence/) — the same gap with an empty argument list
- [callee_comment_optional_args](./../callee_comment_optional_args_prettier_divergence/) — the `?.` split, the same rule one delimiter earlier
- [import_open_paren_comment](./../import_open_paren_comment_prettier_divergence/) — the comment on the *other* side of `import`'s `(`
