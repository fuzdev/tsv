# line_comment_absorbed_prettier_divergence

Prettier absorbs line comments between keyword/paren and block body into the block (or into catch parens).

tsv: preserves comments where the user placed them
Prettier: relocates comments inside the block or catch parameter list

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling across if/else, try/catch, switch, for, while, do-while, labeled statements, and call chains.

## Cases

Both authorings of the same gap, across all four keywords — Prettier absorbs either one, tsv
keeps each where it was written. This mirrors the `if`/`while` `)`→`{` gap exactly.

| gap | trailing (`try // c⏎{`) | own-line (`try⏎// c⏎{`) |
| --- | --- | --- |
| `try` | absorbed into the try block | absorbed into the try block |
| `catch (e)` | absorbed into the catch parens (`catch (⏎\te // c⏎)`) | absorbed into the catch parens |
| bare `catch` | absorbed into the catch block | absorbed into the catch block |
| `finally` | absorbed into the finally block | absorbed into the finally block |

Only **line** comments are covered: an own-line **block** comment normalizes to trailing the
keyword in both formatters (`try⏎/* c */⏎{` → `try /* c */ {`), so it is not a divergence.

The absorbed form (`variant_absorbed.svelte`) is dual-stable: both formatters keep it as-is, so it is a `variant_*`, not the canonical input.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
