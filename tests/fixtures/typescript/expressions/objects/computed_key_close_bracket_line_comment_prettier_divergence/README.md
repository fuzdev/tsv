# Divergence: line comments in a computed-key key→`]` gap

Line comments between a computed property key and its closing `]`
(`[foo // c\n]` and `[bar\n// d\n]`).

Prettier **relocates** the comment out of the brackets — a same-line comment to
the property's own leading line (`// c\n[foo]`), an own-line comment past `]:`
onto the value (`[bar]:\n\t// d\n\t2`); tsv breaks the bracket and preserves each
comment where the author wrote it.

```ts
// prettier (relocates)        // tsv (preserves placement)
const o = {                    const o = {
	// c                            [
	[foo]: 1,                           foo // c
	[bar]:                          ]: 1,
		// d                          [
		2,                                bar
};                                    // d
                                  ]: 2,
                               };
```

A **same-line** comment (`// c`) trails the key; an **own-line** comment (`// d`)
keeps its own line — both inside the broken bracket. A block comment in this gap
(`[baz /* e */]`) stays inline in both formatters and is **not** a divergence.
This is the same preserve-in-place rule tsv applies to every other open/close
delimiter via the shared `build_computed_key_bracket_doc` — the sibling of the
`[`→key gap ([computed_key_open_bracket_line_comment](../computed_key_open_bracket_line_comment_prettier_divergence/))
and the index-signature key-type→`]` gap. See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation and §Comment Position Philosophy. Without the break, a line
comment here would swallow `]` and the value annotation (content loss).
