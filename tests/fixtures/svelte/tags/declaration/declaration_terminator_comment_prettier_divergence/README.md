# declaration_terminator_comment_prettier_divergence

A comment the author wrote between a `{const …}` / `{let …}` tag's last declarator and the
tag's `;` — the declaration's **terminator gap**. Svelte's grammar admits only whitespace
and an optional `;` between the declaration and the closing `}`, so a comment there is
inside the declaration, before the `;`, and that is the only position it can hold. tsv
keeps it there and emits the `;` to carry it.

tsv:

```svelte
{let a /* c1 */;}
{const d = 1 /* c4 */;}
{let l // c8
;}
```

Prettier moves the comment **past** the `;`, which produces a document that is not a
Svelte declaration tag:

```svelte
{let a; /* c1 */}
{const d = 1; /* c4 */}
{let l;} // c8
```

`{let a; /* c1 */}` is rejected by Svelte's own parser (`Expected token }`) — prettier
throws on its own first-pass output, so the committed `output_prettier.svelte` records
those bytes verbatim and there is no `audit_signature.txt`. The `//` spelling reparses but
relocates the comment **out of the tag into template text**, where `// c8` renders on the
page: a different document, not a formatting difference. Both are the `{@const}` bug of
[expr_trailing_line](../../../syntax/comments/expr_trailing_line_prettier_divergence/) one
token over.

The `;` is emitted **because** the comment is there. With the gap bare the tag drops a
terminator it does not need — the `{const q = 1}` and `{let p;}` controls pin the
unchanged rule (a lone declarator with no initializer keeps its `;`, everything else
drops it), so the `;` in the cases above is the comment's own terminator.

Cases: a bare declarator (`a`), a type annotation (`b`) and a definite-assignment marker
(`c`) — the three `emit_semicolon` shapes that already printed a `;`; an initializer
(`d`), two declarators without (`e`, `f`) and with (`g`, `h`) initializers, a
destructuring pattern (`i`, `j`) and an initialized tag inside a block (`o`) — the shapes
that print no `;` of their own and dropped the comment outright; a multi-line block (`k`);
and the `//` spelling at the root (`l`, `m`) and one indent in (`n`), where the comment
runs to end of line so `;}` takes the next line at the tag's own indent.

## Reason

Prettier's placement is a content bug in both spellings — a dead document for a block
comment, a render change for a line comment — so there is no position to adopt. Keeping
the comment where the author wrote it is the only placement that preserves it, reparses,
and is a fixed point. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy);
cataloged in
[conformance_prettier_svelte.md §Svelte: Attributes](../../../../../../docs/conformance_prettier_svelte.md#svelte-attributes).

## Related

- [expr_trailing_line](../../../syntax/comments/expr_trailing_line_prettier_divergence/) — the same rule at every other `{…}` closer, where the terminator is the bare `}`
- [expr_trailing](../../../syntax/comments/expr_trailing_prettier_divergence/) — the block-comment sibling of that fixture
- [declaration_comment](../declaration_comment/) — the declaration tag's interior comment positions, where both formatters agree
