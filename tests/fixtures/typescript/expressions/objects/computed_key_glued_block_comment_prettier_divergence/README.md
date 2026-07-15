# computed_key_glued_block_comment_prettier_divergence

A run of block comments leading an object literal's computed key `[key]`, in the
bracket-break path (a line comment in an in-bracket gap is the only trigger — a computed key
never breaks on width).

The **run itself follows prettier's rule**: a pair the author glued stays glued and the key
breaks below (`a`), and blocks the author put on their own lines keep them (`b`). That is
prettier's `printLeadingComment`, applied through tsv's one shared leading-comment emitter.

## The divergence

Two relocations, neither introduced here, both already sanctioned:

- the `[`-line comment — tsv keeps `// force` on the `[` line; prettier relocates it out to
  the **member's** own leading line (the open-delimiter trailing-comment divergence);
- the in-bracket run — prettier hoists `/* c2 */` out of the brackets entirely
  (`/* c2 */⏎[/* c1 */ key]: 1`), splitting a run the author wrote as one unit and re-binding
  `c2` from the key to the member. tsv keeps both comments where they were written.

Note prettier is not self-consistent across the two cases: it hoists `c2` out in `a` but
leaves both inside the brackets in `b` (`[/* c1 */⏎/* c2 */⏎key]`) — the same comments, the
same position, differing only in whether the author glued them.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
