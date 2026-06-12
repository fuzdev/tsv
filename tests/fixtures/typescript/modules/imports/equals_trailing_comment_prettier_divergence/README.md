# equals_trailing_comment_prettier_divergence

Comments between an `import x = require(...)` / `import x = A.B` module reference
and the terminating `;` are preserved where the user placed them.

**Prettier**: keeps a same-line block comment after the reference in place, but
relocates a line comment past the `;` (`output_prettier.svelte`):

```
import x = require('./a') /* c */;
import y = require('./b'); // c
import z = A.B /* d */;
```

**tsv**: preserves them before `;` — a block comment trails the reference, a line
comment stays on its line with `;` following:

```
import x = require('./a') /* c */;
import y = require('./b') // c
;
import z = A.B /* d */;
```

Per Comment Position Philosophy (and the before-semicolon divergence), the user's
chosen position is preserved. The block cases are dual-stable; only the line
comment diverges. Same mechanism as `source_trailing_comment_prettier_divergence`.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
