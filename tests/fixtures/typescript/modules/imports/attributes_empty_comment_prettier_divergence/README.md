# attributes_empty_comment_prettier_divergence

Comments around an **empty** import-attributes clause (`with {}`) are preserved
where the user placed them. The empty `with {}` itself is kept (both formatters);
only the comment position diverges.

**Prettier** (`output_prettier.svelte`): relocates every comment to before `with`
and emits `with {}`:

```
import a from 'a' /* c1 */ with {};
import b from 'b' /* c2 */ with {};
import c from 'c' /* c3 */ with {};
import d from 'd' /* c4 */ with {};
```

**tsv**: preserves each comment where the user wrote it:

```
import a from 'a' /* c1 */ with {};
import b from 'b' with /* c2 */ {};
import c from 'c' with {/* c3 */};
import d from 'd' with {} /* c4 */;
```

The source→`with` block comment (c1) is dual-stable — both keep it in place. The
`with`→`{` (c2), inside-braces (c3), and after-`}` (c4) comments diverge: prettier
relocates them before `with`, tsv keeps them in place. Per Comment Position
Philosophy — the empty-`with` analog of the `with_keyword_comment` and
`source_trailing_comment` import divergences.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
