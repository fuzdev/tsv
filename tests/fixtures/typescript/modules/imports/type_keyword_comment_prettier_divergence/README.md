# type_keyword_comment_prettier_divergence

Comments around the `type` keyword of a type-only import with named specifiers —
between `import` and `type`, or between `type` and the specifier braces — are
preserved where the user placed them.

**Prettier**: relocates them *into* the braces, as the first specifier's leading
comment (`output_prettier.svelte`); a line comment also expands the braces:

```
import type { /* c1 */ A } from './a';
import type { /* c2 */ B } from './b';
import type {
	// c3
	C
} from './c';
```

**tsv**: preserves them where the user placed them — a block comment trails inline,
a line comment stays on its own line with the following token after it:

```
import /* c1 */ type { A } from './a';
import type /* c2 */ { B } from './b';
import // c3
type { C } from './c';
import type // c4
{ D } from './d';
```

Per Comment Position Philosophy, the user's chosen position is preserved. The
non-empty counterpart of `empty_type_keyword_comment_prettier_divergence` (empty
`{}`, where prettier instead relocates after `from`).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
