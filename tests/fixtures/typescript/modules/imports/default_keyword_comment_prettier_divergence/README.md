# default_keyword_comment_prettier_divergence

Comments in a default import's header — between `import` and the binding, around an
optional `type` keyword — are preserved where the user placed them.

**Prettier**: keeps a comment that is already adjacent to the binding, but relocates
a comment between `import` and `type` to the binding side of `type`; line comments
keep the continuation flat (`output_prettier.svelte`):

```
import /* c1 */ Foo from './a';
import type /* c2 */ Bar from './b';
import type /* c3 */ Baz from './c';
import type // c4
Qux from './d';
import // c5
Quux from './e';
import type // c6
Corge from './f';
```

**tsv**: preserves each comment where the user placed it; a line comment forces the
following token onto the next line, indented one level (the uniform module-header
rule):

```
import /* c1 */ Foo from './a';
import /* c2 */ type Bar from './b';
import type /* c3 */ Baz from './c';
import // c4
	type Qux from './d';
import // c5
	Quux from './e';
import type // c6
	Corge from './f';
```

The block `import`→`type` gap (c2) diverges on position. The three line comments
(c4, c5, c6) diverge on indentation — tsv indents the continuation one level where
Prettier keeps it flat (an indent-only divergence). The block `import`→binding (c1)
and `type`→binding (c3) positions are dual-stable (both formatters keep them). Per
Comment Position Philosophy. Sibling of the empty/named
`type_keyword_comment_prettier_divergence` fixtures.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
