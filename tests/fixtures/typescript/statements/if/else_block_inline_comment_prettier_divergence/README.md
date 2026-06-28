# else_block_inline_comment_prettier_divergence

Inline block comments between an `if` block's closing `}` and the `else` keyword
(`} /* inline */ else`).

A **single** inline block comment stays on the `} … else` line in both
formatters — no divergence. **Multiple** inline block comments diverge under
prettier 3.9: prettier drops every comment after the first onto its own line
(`} /* c1 */\n/* c2 */ else`), while tsv keeps them all inline on the `} … else`
line (`} /* c1 */ /* c2 */ else`).

```ts
// prettier 3.9 (splits after the first)   // tsv (keeps all inline)
} /* c1 */                                  } /* c1 */ /* c2 */ else {
/* c2 */ else {                                 fn2();
	fn2();                                  }
}
```

## Reason

tsv treats the author's same-line placement as intentional, keeping consecutive
inline block comments cuddled on the `} … else` line. Consistent with tsv's
handling across other control-flow statements.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
