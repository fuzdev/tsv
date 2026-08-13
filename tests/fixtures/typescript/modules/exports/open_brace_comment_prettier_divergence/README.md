# open_brace_comment_prettier_divergence

A comment trailing an export specifier list's opening `{` on the same line
(e.g. `export { // c` or `export { /* c */`) is preserved on the `{` line.
Prettier relocates it to its own line as the first specifier's leading comment.
Applies to both re-exports (`export { … } from '…'`) and local exports
(`export { … }`).

tsv: keeps the comment trailing `{` where the user placed it
Prettier: moves the comment down to its own line

```
// tsv                          // prettier
export { // c1                  export {
	a                                  // c1
} from './a';                      a
                                 } from './a';
```

## Reason

tsv treats user comment placement as intentional (see Comment Position
Philosophy). A comment the author parked after `{` is a trailing comment on
that line; relocating it to its own line is a syntactic-position move. tsv
preserves it in place, which is also idempotent in a single pass (Prettier's
relocation is its own canonical form). When the author instead writes the
comment on its own line, both formatters keep it there — the two positions are
dual-stable. An empty specifier list (`export {} from 'x'`) has no first
specifier and is unaffected; only the expanding cases (a line comment after
`{`, or own-line content forcing a break) diverge.

This is the export-specifier member of the open-delimiter family, alongside the
sibling import-specifier case. Import/export specifier lists share the multiline
comma-list builder (`build_hardline_comma_list`, `statements/modules.rs`); it
routes through the shared `Printer::delimiter_line_comment_prefix` helper used
by the object/array literal, destructuring, block-body, `namespace`/`module`,
class/interface/enum body, and type literal cases.

## Author blank after the pulled comment

An author blank line between the pulled comment and the first specifier does not
survive — tsv prints the specifier directly under the `{` line. That is a
*consequence* of the position above, not a second choice: both formatters
discard a blank in the **leading gap** (an opening delimiter's line → the first
item), and Prettier's relocation moves the comment out of that gap, making the
blank an inter-item one it then keeps. The authored shape is pinned by
`unformatted_ours_blank_after_comment.svelte`, which tsv normalizes back to
`input.svelte`.

⚠️ Unlike the rest of the open-delimiter family, Prettier's form here is **not**
dual-stable — tsv rewrites it to a third form
(`divergent_variant_blank_after_comment.svelte`). The specifier-list builder
also drops an author blank after a **leading own-line** comment:

```
export {          export {
	// c        →       // c
                        a
	a               };
};
```

Nothing is pulled onto the `{` line there and both formatters agree on the
comment's position, so that second drop is a separate behavior of the
specifier-list builder — not a consequence of this divergence, and not
sanctioned by it. Every other container in the family preserves that blank.

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
