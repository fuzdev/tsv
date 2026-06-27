# source_trailing_comment_prettier_divergence

A block comment between an import's source literal (or `with {...}` attributes)
and the terminating `;` trails **after** the `;` (`import { a } from './a'; /* c */`),
and a same-line trailing line comment trails after the `;` too (`; // 1`) — both
match prettier 3.9. `input.svelte` is dual-stable: tsv and prettier both keep it.

The divergence is narrower: an **own-line line comment that follows a trailing
line comment** (the `// 2` after `// 1`). tsv keeps `// 2` attached to the
*preceding* import (a blank line falls after it, before the next statement);
prettier promotes `// 2` to a *leading* comment of the *following* statement (the
blank line falls before it). The `unformatted_ours_*` variants — the same code
authored with the comments before the `;` — show the split: tsv reflows them to
`input.svelte`, prettier reflows the `// 2` to lead the next statement, so they
are `unformatted_ours_*` (tsv-only normalization), not `unformatted_*`.

Per Comment Position Philosophy, tsv keeps an own-line comment with the statement
the author wrote it under rather than promoting it across the statement boundary.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
