# from_comment_prettier_divergence

A comment in the gap between an export's specifier braces (`}`) and the `from` keyword is
preserved where the user placed it.

**Prettier** (`output_prettier.svelte`): relocates the comment _into_ the braces as the last
specifier's trailing comment — a block comment inline, a line comment expanding the braces
multiline:

```
export {a /* c1 */} from './a';
export {
	b, // c2
} from './b';
export {c, d /* c3 */} from './c';
```

**tsv**: preserves each comment between `}` and `from`:

```
export {a} /* c1 */ from './a';
export {b} // c2
from './b';
export {c, d} /* c3 */ from './c';
```

Per Comment Position Philosophy. The into-braces relocation matches the sibling import
`from_comment` and `type_keyword_comment` fixtures; the no-`from` analog (`}`→`;`) is
`close_brace_comment`.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
