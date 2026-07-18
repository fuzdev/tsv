# layer_escaped_whitespace_prettier_divergence

An at-rule **prelude** that ends in an escaped whitespace (`@layer a\ ;`).

`\ ` is a valid escape whose escaped code point **is that space** (css-syntax-3 §4.3.4
/ §4.3.7), so the layer is named `a ` and that space is *content*, not padding. The `;`
after it terminates the at-rule.

tsv preserves it, so `input.svelte` formats to itself.

**Prettier trims it as if it were padding**, stranding the backslash onto whatever
follows:

| source | prettier | consequence |
| --- | --- | --- |
| `@layer a\ ;` | `@layer a\;` | `\;` escapes the terminator — the at-rule never ends |
| `@layer b\ {` | `@layer b\ {` | correct — the `{` is separated by the payload space, so nothing abuts the `\` |

The first is a real corruption, and its shape depends on what follows:

- **alone**, prettier's output does not parse at all — tsv rejects `@layer a\;` with
  `Expected '{' or ';' after 'at-rule prelude'`, because the prelude runs to end-of-file
  looking for a terminator;
- **in context** (as in this fixture) it parses, but *wrongly*: the escaped `;` lets the
  prelude swallow everything up to the next `{`, so the two `@layer` rules **merge into
  one** — `input.svelte` parses to two `Atrule` nodes, `output_prettier.svelte` to one,
  whose prelude is `a\;\n\n\t\n\t@layer b\`.

tsv declines to reproduce it: **its format→re-parse invariant outranks matching
prettier.** Emitting output that does not parse — or that silently merges two rules — is
never the defensible side, whatever the reference formatter does.

The block form also shows the separator rule: the payload space already separates the
prelude from the `{`, so tsv does not add a second one (`@layer b\ {`, not
`@layer b\  {`) — the same absorption the selector path applies before `{` and `,`.

This is the at-rule face of the rule cataloged for ordinary values. See
[css/values/escaped_whitespace](../../values/escaped_whitespace_prettier_divergence/) for
the declaration/function faces and
[url_escaped_whitespace](../../values/functions/url_escaped_whitespace_prettier_divergence/)
for `url()`.

See [conformance_prettier.md §CSS: At-Rules](../../../../../docs/conformance_prettier.md#css-at-rules).
