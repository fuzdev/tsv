# `await`/`new` forced continuation, at the print-width boundary

The width consequence of
[await_new_operand_line_comment](../await_new_operand_line_comment_prettier_divergence/):
tsv's continuation indent moves the operand two columns in, so the tail is measured
from there. Same source, two answers — prettier keeps all three lines flat at its own
flush column (98/99/99), tsv breaks the two that cross 100 at the continuation's.

- **c1** — exactly **100** at the continuation's indent: stays flat.
- **c2** — the same line one character longer, **101**: the argument list breaks, and
  it breaks at the **continuation's** indent (arguments one level in from `Foo(`, the
  `)` back out). That is the claim the boundary exists to make — the whole tail rides
  the continuation, not the callee alone, so a broken argument list cannot render at
  the outer column.
- **c3** — the `await` operand crosses the same boundary the same way.

Both formatters are idempotent on their own form here; the divergence is the indent
(§Uniform Forced-Continuation Indent) plus the two columns it costs.

See [conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
