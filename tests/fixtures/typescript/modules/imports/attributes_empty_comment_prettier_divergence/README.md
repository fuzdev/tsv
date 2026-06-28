# attributes_empty_comment_prettier_divergence

Comments around an **empty** import-attributes clause (`with {}`) are preserved
where the user placed them. The empty `with {}` itself is kept (both formatters);
only the comment position diverges.

**Prettier** (`output_prettier.svelte`): relocates the `with`→`{` and
inside-braces comments to before `with` (c2, c3), keeps the source→`with` comment
in place (c1), and trails the after-`}` comment past the `;` (c4):

```
import a from 'a' /* c1 */ with {};
import b from 'b' /* c2 */ with {};
import c from 'c' /* c3 */ with {};
import d from 'd' with {}; /* c4 */
```

**tsv**: preserves each comment where the user wrote it, except it trails the
after-`}` comment past the `;` to match prettier (c4 — the lossless
trail-past-a-separator carve-out):

```
import a from 'a' /* c1 */ with {};
import b from 'b' with /* c2 */ {};
import c from 'c' with {/* c3 */};
import d from 'd' with {}; /* c4 */
```

The source→`with` block comment (c1) is dual-stable and the after-`}` comment (c4)
agrees in both. Only the `with`→`{` (c2) and inside-braces (c3) comments diverge:
prettier relocates them before `with`, tsv keeps them in place. Per Comment
Position Philosophy — the empty-`with` analog of the `with_keyword_comment` and
`source_trailing_comment` import divergences.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
