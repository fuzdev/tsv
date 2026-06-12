# with_keyword_comment_prettier_divergence

A comment in an import's attributes header — between the source and the `with` keyword, or
between `with` and the attributes `{` — is preserved where the user placed it.

**Prettier** (`output_prettier.svelte`): keeps a block comment already before `with` in
place, floats a line comment past the `;`, and relocates a `with`→`{` block comment back to
before `with`:

```
import a from './a' /* c1 */ with {type: 'json'};
import b from './b' with {type: 'json'}; // c2
import c from './c' /* c3 */ with {type: 'json'};
import d from './d' with {type: 'json'}; // c4
```

**tsv**: preserves each comment where the user placed it:

```
import a from './a' /* c1 */ with {type: 'json'};
import b from './b' // c2
with {type: 'json'};
import c from './c' with /* c3 */ {type: 'json'};
import d from './d' with // c4
{type: 'json'};
```

The source→`with` block comment (c1) is dual-stable — both formatters keep it in place. The
source→`with` line comment (c2) and the `with`→`{` line comment (c4) float past `;` in
Prettier (the before-semicolon/float-out rule); the `with`→`{` block comment (c3) relocates
to before `with`. Per Comment Position Philosophy. Sibling of the import `from_comment`
divergence (the gap one token earlier).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
