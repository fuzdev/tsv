# source_trailing_comment_prettier_divergence

Comments between an import's source literal (or `with {...}` attributes) and the
terminating `;` are preserved where the user placed them.

**Prettier**: keeps a same-line block comment after the source in place, but
relocates a line comment past the `;`, and relocates a block comment after
import attributes *inside* the attribute braces (`output_prettier.svelte`):

```
import { a } from './a' /* c */;
import { b } from './b'; // 1
// 2
import c from './c' with { type: 'json' /* c */ };
```

**tsv**: preserves them where the user placed them — a block comment trails the
source / attribute `}`, line comments stay on their own line with `;` following:

```
import { a } from './a' /* c */;
import { b } from './b' // 1
// 2
;
import c from './c' with { type: 'json' } /* c */;
```

Per Comment Position Philosophy (and the before-semicolon divergence), the user's
chosen position is preserved. The bare-source block case is dual-stable (both
formatters keep it in place); the line comment and the post-attributes block
comment diverge.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
