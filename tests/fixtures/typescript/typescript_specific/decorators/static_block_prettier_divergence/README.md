# Decorated static block: the decorator list is kept

A decorator list in front of a class `static {}` block (`@a(b) static {}`). acorn-typescript
parses it and keeps the list on the block: the `StaticBlock` carries `decorators` after
`body`, and its span opens at the first decorator, as any decorated member's does. tsc's
parser builds the block too and reports **TS1206** ("Decorators are not valid here.") from
its grammar checker, the deferred side of tsv's reject-vs-defer line.

- **prettier**: parses it and prints `static {}`, dropping the whole decorator list on the
  first pass (`output_prettier.svelte`). The authored decorators are gone from the output.
- **tsv**: keeps the list and prints it in the layout of any decorated member: inline when
  authored inline (`@a(b) static {}`), one decorator per line when authored on their own
  lines.

See
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(Decorated static block) and
[conformance_prettier.md §Reasons tsv Differs](../../../../../../docs/conformance_prettier.md#reasons-tsv-differs)
(`◆content_preservation`).
