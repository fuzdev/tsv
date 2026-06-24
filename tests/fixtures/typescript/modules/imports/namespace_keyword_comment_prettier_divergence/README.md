# namespace_keyword_comment_prettier_divergence

Comments in a namespace import's header — between `import` and `* as ns`, around an
optional `type` keyword, or between `*` and `as` — are preserved where the user
placed them.

**Prettier**: keeps a comment already adjacent to `*`, but relocates a comment between
`import` and `type` to after `type`, and a comment between `*` and `as` to after `as`;
line comments keep the continuation flat (`output_prettier.svelte`):

```
import /* c1 */ * as ns1 from './a';
import type /* c2 */ * as ns2 from './b';
import type /* c3 */ * as ns3 from './c';
import type // c4
* as ns4 from './d';
import * as /* c5 */ ns5 from './e';
import // c6
* as ns6 from './f';
import type // c7
* as ns7 from './g';
```

**tsv**: preserves each comment where the user placed it; a line comment forces the
following token onto the next line, indented one level (the uniform module-header
rule):

```
import /* c1 */ * as ns1 from './a';
import /* c2 */ type * as ns2 from './b';
import type /* c3 */ * as ns3 from './c';
import // c4
	type * as ns4 from './d';
import * /* c5 */ as ns5 from './e';
import // c6
	* as ns6 from './f';
import type // c7
	* as ns7 from './g';
```

The block `import`→`type` (c2) and `*`→`as` (c5) gaps diverge on position — prettier
relocates the former to after `type`, the latter to after `as`. The line comments
(c4, c6, c7) diverge on indentation — tsv indents the continuation one level where
Prettier keeps it flat. The block `import`→`*` (c1) and `type`→`*` (c3) positions are
dual-stable (both formatters keep them). Per Comment Position Philosophy.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
