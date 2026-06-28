# type_alias_equals_prettier_divergence

A line comment between a type alias's `=` and its value type (`type A = // c\n\tB`).

**tsv** keeps the comment trailing the `=`, with the value type dropped to a
continuation line indented one level. **Prettier** relocates the comment to its
own line after the `=`. Both forms are stable under their respective formatters.

## Reason

Per Comment Position Philosophy, tsv keeps the comment after the `=` (the uniform
forced-continuation indent) rather than floating it to its own line. The fixture
covers simple, union, and intersection value types and two stacked line comments
(which stay distinct in both formatters). A **block** comment after `=`
(`type B = /* comment */ C;`) stays inline in both formatters and is not a
divergence — only a line comment forces the value onto the next line, and the two
formatters disagree on which line the comment lands.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Uniform Forced-Continuation Indent.
