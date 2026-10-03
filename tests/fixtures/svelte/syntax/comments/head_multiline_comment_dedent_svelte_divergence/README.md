# Parser divergence: a comment on a manufactured line is dedented by the document's line

acorn's `onComment` (`svelte/.../1-parse/acorn.js`) dedents a multi-line block
comment: it takes the `[ \t]` run opening the comment's line and strips one copy
of it from the start of every line of the `value`. Svelte means that dedent —
esrap prints `value` back — and tsv keeps it. What differs is **which line** the
run is read from.

Svelte reads it out of the string its reader handed acorn, and three of the
template readers hand acorn a rewritten one:

| reader | the string acorn got |
| --- | --- |
| `read_type_annotation`, a block binding's `: T` | the prefix blanked to spaces, then `_ as ` over the five UTF-16 code units ending at the colon |
| `read_pattern`, a destructuring binding | the prefix blanked to spaces, its first space dropped, then `(pattern = 1)` |
| the `{#snippet}` head | the prefix with only its NON-whitespace blanked |

So on the line where that manufacture ends, Svelte measures a run the document
does not hold: an indented head's tab is a blanked space (or, behind the
snippet prelude, the tab plus the blanked columns after it), which matches none
of the comment's lines, and the author's tab survives into the `value`. tsv
measures the run on the **document's** line — the same `\n`-only walk-back and
the same `[ \t]` class, over the author's bytes — so the tab comes off, as it
would on any other line.

```
	{#each xs as x: /*
	 c1 */ number}

// Svelte value      // tsv value
"\n\t c1 "           "\n c1 "
```

| comment | where it opens | Svelte | tsv |
| --- | --- | --- | --- |
| `c1` | `{#each xs as x:` annotation | `"\n\t c1 "` | `"\n c1 "` |
| `c2` | `{@const { a = …` destructure | `"\n\t\t c2 "` | `"\n c2 "` |
| `c3` | `{@const b = …` init (raw template) | `"\n c3 "` | `"\n c3 "` |
| `c4` | `{#await p then { c = …` destructure | `"\n\t c4 "` | `"\n c4 "` |
| `c5` | `{#snippet s(d = …` head | `"\n\t c5 "` | `"\n c5 "` |
| `c6` | `{expr …}` (raw template) | `"\n c6 "` | `"\n c6 "` |

`c3` and `c6` are the controls: `read_expression` hands acorn the raw template,
so the line Svelte measures is the document's, and both parsers agree. The
difference is in each `value` alone — on the root `comments` entry and the
attached `leadingComments` copy alike — and nowhere else in the tree.

The `<!-- prettier-ignore -->` keeps the triggers alive: both formatters reflow
every one of these heads onto a line the manufacture no longer reaches. The
fourth manufactured reader, `read_script` (a `<script>` body's prefix blanked),
cannot be a fixture at all — prettier reformats a script body through an ignore
directive, and both formatters put its first statement below the tag — so it is
pinned, with the spellings no formatter leaves standing, by
[`tests/comment_dedent_document_line.rs`](../../../../../comment_dedent_document_line.rs).

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md#comment-attachment-differences) §Comment Attachment Differences.
