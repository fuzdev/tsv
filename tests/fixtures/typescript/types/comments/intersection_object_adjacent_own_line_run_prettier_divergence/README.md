# Divergence: own-line comment run leading an object-adjacent intersection member

An own-line comment run in the `&`→member gap where **both** neighbouring members
are object types (`{ x: 1 } &⏎/* c1 */⏎{ y: 2 }`) — the boundary prettier's
`printIntersectionType` hugs. The line comment above (`// k`) is only there to route
the intersection through the comment-aware printer; the shape under test is the
second boundary.

tsv keeps the run where the author wrote it — on its own line, above the member —
and indents the member with it (uniform forced-continuation indent):

```ts
{ x: 1 } &
/* c1 */
{ y: 2 };
```

Both cases take the same answer: a single block (`A`) and an author-glued pair
(`B`), the pair staying on one line as authored.

## No prettier oracle — prettier never converges

Prettier has **no stable form** here. Its arm chain answers a both-objects boundary
with `[" & ", doc]` *before* it asks `hasLeadingOwnLineComment`, so the comment's
own-line-ness never holds the break its position needs — and the comment re-binds on
every pass. Pass 1 pulls the run onto the operator's line and leaves the member at a
shallower indent than its siblings (`{ x: 1 } & /* c1 */⏎{ y: 2 }`); pass 2 carries
the run back **across** the `&` to trail the previous member and collapses the
boundary entirely (`{ x: 1 } /* c1 */ & { y: 2 }`). Recorded with a
`prettier_nonconvergent.txt` marker, live-verified by the validator (rule F5).

tsv is stable and lossless on the same input, and the position it keeps is the one
[union_infix_pipe_line_comment](../union_infix_pipe_line_comment_prettier_divergence/)
states for the union: a comment written after the separator belongs to the member
side, not to the member behind it.

## The bug this guards

Emitting the run only inside the breaking arm of the boundary's break/hug choice
**drops** the comment outright: the hugging arm prints the member and nothing else —
`{ x: 1 } &⏎/* c */ { y: 2 }` comes out
`{ x: 1 } & { y: 2 }`, any run length. The glued-forward spelling is pinned by
[intersection_object_adjacent_leading_run](../intersection_object_adjacent_leading_run/);
this fixture pins the own-line spelling, which reaches the same emitter through the
break arm. See
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation and
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment Position Philosophy, §Uniform Forced-Continuation Indent.
