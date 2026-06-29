# constraint_default_own_line_block_comment_prettier_divergence

An **own-line block** comment after a type parameter's `extends` constraint or `=`
default, before the bound type (`<T extends⏎/* c */⏎U>`). The comment forces the
`<…>` to expand; tsv hangs the bound type on its own line under the keyword:

```
type F<
	T extends
		/* c */
		U
> = T;
```

**Prettier** pulls the comment onto the keyword line, then on a second pass
relocates it *before* the keyword and collapses the whole `<…>`
(`type F<T /* c */ extends U> = T`) — non-idempotent, so `audit_signature.txt`
pins the chain.

A **same-line** block comment trailing the keyword with the type below
(`<T extends /* c */⏎U>`, no preceding newline) likewise keeps the comment
trailing `extends`/`=` and the type hanging (the `<…>` still expands); prettier
relocates it before the keyword and collapses. A block comment **glued** to the
type (`<T extends /* c */ U>`) stays inline in both formatters and is not a
divergence.

Per Comment Position Philosophy, tsv keeps the comment where the author wrote it
(after `extends`/`=`) and applies the shared keyword→value hang
(`append_keyword_value_line_comments`), the same as the prefix type-operator and
`as`/`satisfies` gaps. Covers both the `extends` constraint and the `=` default.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
