# default_keyword_comment_prettier_divergence

Comments in a default import's header — between `import` and the binding, around an
optional `type` keyword — are preserved where the user placed them.

**Prettier**: keeps a comment that is already adjacent to the binding, but relocates
a comment between `import` and `type` to the binding side of `type`
(`output_prettier.svelte`):

```
import /* c1 */ Foo from './a';
import type /* c2 */ Bar from './b';
import type /* c3 */ Baz from './c';
import type // c4
Qux from './d';
```

**tsv**: preserves each comment where the user placed it:

```
import /* c1 */ Foo from './a';
import /* c2 */ type Bar from './b';
import type /* c3 */ Baz from './c';
import // c4
type Qux from './d';
```

Only the `import`→`type` gap diverges (c2, c4); the `import`→binding (c1) and
`type`→binding (c3) positions are dual-stable (both formatters keep them). Per
Comment Position Philosophy. Sibling of the empty/named
`type_keyword_comment_prettier_divergence` fixtures.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
