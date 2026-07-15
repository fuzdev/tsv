# Divergence: `export default`→first-decorator glued comment (preserve the binding)

A block comment **glued to the first decorator** of a decorated default-exported class
(`export default⏎/* c */ @dec⏎class {}`). tsv keeps it glued to `@dec`; prettier **relocates** it back
past `default` and then inserts blank lines below it.

```ts
// tsv (preserve the binding)   // prettier (relocate + blanks, 3 passes)
export default                  export default /* c */
/* c */ @dec
                                (blank)
class {}                        (blank)
                                @dec
                                class {}
```

**Why tsv preserves:** the comment is glued to `@dec`, so the parser binds it there
(`Comment::owned_by_node` — every glued block comment is owned). Gluing it back to `@dec` on output is
what keeps that binding visible. The complementary authoring is preserved just as faithfully: a comment
the author left on its own line after the keyword (`export default /* c */⏎@dec`) is *not* glued, so it
is not owned, and the keyword→decorator gap emits it there — see the sibling
[default_keyword_decorator_comment](../default_keyword_decorator_comment_prettier_divergence/), which is
the gap on the *other* side of `default`. Two authorings, two positions, each preserved.

Prettier is **non-idempotent** here: `output_prettier.svelte` is its first-pass output, and it then adds
one blank line per pass, saturating at two (its max-consecutive-blank-lines rule). `audit_signature.txt`
pins that chain.

**Why this fixture exists:** tsv **dropped** this comment outright. Ownership takes a comment out of the
positional model — every gap emitter correctly skips it, because the owning node is supposed to print
it — but the decorated-`export default` path builds the class expression directly rather than through
`build_expression_doc`, so nothing claimed it. It is the owned-comment hazard in tsv's own words: *an
owned comment nothing prints is a dropped comment*. The fix claims it on this seam
(`prepend_owned_leading_comment_at`), exactly as the arrow-reassembly paths do.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
