# const_annotation_comment_svelte_divergence

A `{@const}` that carries a **type annotation** makes Svelte list every comment
from the `:` to the tag close **twice** in `root.comments`, and attach both
copies. tsv lists each comment once.

```
{@const a1: /* c1 */ T = expr}

// Svelte root comments      // tsv root comments
[c1, c1]                     [c1]
```

## Reason

Svelte's `read_type_annotation` (`1-parse/read/context.js`) tricks acorn into
parsing the annotation by building `_ as <annotation> = <init>`. That parse is an
`AssignmentExpression`, so it hits the reader's own "gets mangled — fix it"
branch and is **re-parsed** over the slice up to the `=`. The throwaway first
parse is discarded, but its `onComment` has already pushed every comment it
scanned — the whole annotation-to-tag-close region — into the shared
`root.comments`. The two real parses then push their own copies, giving the
order [pass 1: all, pass 2: annotation region, pass 3: init region]. Because
`add_comments` re-filters the *whole accumulated* array rather than its own
parse's pushes, the duplicates are attached too.

The trigger is the annotation's **presence**, not a comment's position: `a3`
carries no annotation comment at all and its **init** comment is still doubled,
while `a4` — the same init comment with no annotation on the binding — is listed
once by both parsers. That pair is the control.

tsv parses the annotation as part of the binding, once, so each comment exists
once and attaches once. The distinct-comment set is identical, `ast_diff`
confirms semantic equivalence, and the formatter — which locates comments by
position — is unaffected and matches prettier on every case here.

## The second claim: the two copies need not land on the same NODE

`a1`'s two copies both land on `T` (`leadingComments: [c1, c1]`), so the
divergence there reads as a longer list. `a6` puts the same comment at a **union
seam**, where they split: `add_comments` walks with the whole accumulated array
as its queue, so `A` takes the first copy as a `trailingComments` (the gap to the
comment is `/^[,) \t]*$/`) and `B` then finds the second still queued and ahead of
its own start, taking it as a `leadingComments`. Canonical therefore carries an
attachment tsv has **nowhere**, not merely one more entry in a list tsv also
writes.

`a0` is the null control, and it is what makes the attachment the duplicate's
and not the seam's: the same comment at the same seam in a plain `<script>`,
which Svelte reads **once**. There canonical attaches it to `A` alone — exactly
what tsv emits at `a6` — so a single copy of the comment produces tsv's answer
under canonical's own walk.

## The third claim: which line the DEDENT reads

`a5` is a multi-line block comment inside the annotation, opening on the line
where `read_type_annotation`'s synthetic source ends. acorn's `onComment`
dedents such a comment by the `[ \t]` run opening its line **in the string acorn
was given** — the template blanked to spaces, with `_ as ` overwriting the five
UTF-16 code units ending at the colon — so to Svelte the tab opening `a5`'s line
is a blanked space, and its `value` keeps the tab (`"\n\t c5 "`). tsv measures
the run on the **document's** line, which is that tab, so the tab comes off
(`"\n c5 "`) — on the root `comments` entry and on the attached
`leadingComments` copy alike.

That is a second divergence in this fixture, cataloged on its own (the
manufactured-line dedent, in the same section). `a5` is the one spelling of it
that is a format fixed point unfrozen; the other template spellings ride the
`<!-- prettier-ignore -->`-frozen
[`head_multiline_comment_dedent_svelte_divergence`](../../../syntax/comments/head_multiline_comment_dedent_svelte_divergence/)
fixture, and the `<script>` reader and the spellings no formatter leaves
standing are pinned by
[`tests/comment_dedent_document_line.rs`](../../../../../comment_dedent_document_line.rs).

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §Comment Attachment Differences.
