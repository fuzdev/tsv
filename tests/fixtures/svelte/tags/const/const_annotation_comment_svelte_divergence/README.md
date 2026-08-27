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

## The second claim: which source the DEDENT reads

`a5` is a multi-line block comment inside the annotation, and it pins the other
thing `read_type_annotation`'s synthetic source decides. acorn's `onComment`
dedents such a comment by the `[ \t]` run opening its line **in the string acorn
was given** — and that string is the template blanked to spaces up to five bytes
behind the type, with `_ as ` spliced over those five. So the tab opening `a5`'s
line is not indentation acorn can see (it is a blanked space, and the run before
it ends at the `_`), and the comment's `value` keeps its tab where reading the
document would have stripped it.

Both parsers answer that identically — the value is a **match**, on the root
`comments` entry and on the attached `leadingComments` copy alike. What still
diverges is only the duplication above. The spellings this fixture cannot hold
unfrozen — a head the formatter would join back onto one line, and the other
three synthetic sources (`read_script`, `read_pattern`, `{#snippet}`'s prelude),
none of which is a format fixed point — are pinned by
[`tests/comment_dedent_manufactured_source.rs`](../../../../../comment_dedent_manufactured_source.rs)
and, for the template readers, by the `<!-- prettier-ignore -->`-frozen
[`head_multiline_comment_dedent`](../../../syntax/comments/head_multiline_comment_dedent/)
fixture.

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §Comment Attachment Differences.
