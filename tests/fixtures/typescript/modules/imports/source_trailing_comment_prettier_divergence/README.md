# source_trailing_comment_prettier_divergence

A same-line block comment after an import's source literal (or `with {...}`
attributes), and a same-line trailing line comment, both trail **after** the `;`
in both formatters (`import { a } from './a'; /* c */`, `import { b } from './b'; // 1`)
— tsv matches prettier here, no divergence. `input.svelte` is dual-stable: tsv
and prettier both keep it.

The divergence is one statement: an **own-line line comment in pre-`;` position
followed by a blank line**. When the comment sits between a statement's last token
and its terminating `;` and a blank line follows, prettier floats the blank
*above* the comment — promoting `// 2` to lead the *following* statement — while
tsv keeps the blank *below* it, so `// 2` stays attached to the statement it was
written under. The `unformatted_ours_*` variants — the same code authored with the
comments before the `;` — pin the split: tsv reflows them to `input.svelte`,
prettier reflows `// 2` to lead the next statement.

The trailing `; // 1` and the import context are **incidental** to the trigger —
the divergence holds for a bare own-line comment (no preceding `// 1`) and for any
statement, not just imports (a plain `const` with the comment before its `;` and a
blank line after diverges the same way). It is the blank-line sibling of
[`line_before_semicolon`](../../../syntax/comments/line_before_semicolon/) (own-line
comment before `;`, *no* blank → both formatters agree); adding the blank line is
what flips it.

That promoted form is dual-stable — both formatters keep it as-is — so it is
pinned as `variant_promoted.svelte`, not the canonical input.

Per Comment Position Philosophy, tsv keeps an own-line comment with the statement
the author wrote it under rather than promoting it across the statement boundary.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
