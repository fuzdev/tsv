# type_keyword_comment_long_prettier_divergence

The width axis of `type_keyword_comment_prettier_divergence`: a header-gap comment on
a **lone** named specifier restores breaking, so the braces expand past print width
where a comment-free lone specifier would stay on one line.

Prettier's `printModuleSpecifiers` refuses to break a single braced specifier
(`canBreak`) unless a second specifier, a leading default/namespace binding, or a
comment *on a specifier* is present. Its comment attachment reaches past the braces:
a comment in either header gap — `import`→`type` or `type`→`{` — becomes the
specifier's leading comment, so either one restores breaking. tsv mirrors that rule
while keeping the comment where the author wrote it.

**Prettier**: relocates the comment into the braces (`output_prettier.svelte`), then
breaks at 101:

```
import type { /* c */ AAAA…A } from './mod';
import type {
	/* c */ BBBB…B
} from './mod';
```

**tsv**: preserves the comment in its authored gap, and breaks at the same boundary:

```
import /* c */ type { AAAA…A } from './mod';
import /* c */ type {
	BBBB…B
} from './mod';
```

The control — a lone specifier with **no** comment, at the same 101 — stays on one
line in both formatters, which is what makes the comment the operative variable
rather than the width (the comment-free rule itself is pinned by
[single_specifier_long](../single_specifier_long/)). Only the comment's position
diverges; where the braces break does not.

The export sibling is
[modules/exports/type_keyword_comment_long](../../exports/type_keyword_comment_long_prettier_divergence/).

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
