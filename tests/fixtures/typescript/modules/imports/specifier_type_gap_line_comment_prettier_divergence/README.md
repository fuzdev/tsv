# Divergence: named-import-specifier `type` modifier gap (preserve, one indent level)

A *line* comment between a specifier's own `type` modifier and the name it modifies
(`{ type //c⏎A }`). tsv keeps it where the author wrote it and drops the tail (the name, and
any `as <local>` after it) to a continuation line at **one** indent level; prettier
**relocates** every such comment to lead the whole specifier and stacks them flush before it.

```ts
// tsv (preserve)          // prettier (relocate to lead the specifier)
import {                   import {
	type //c                  //c
		A                     type A
} from 'x';                } from 'x';
```

The modifier-gap analog of the same specifier's
[rename `as` gaps](../specifier_as_gap_line_comment_prettier_divergence/), and the same shared
renamed-specifier printer (`build_renamed_specifier_doc`) routes both through the shared
header-gap continuation helpers — so a `//` can't swallow the name (the gap previously had no
emitter at all, and every comment in it was **dropped**). A same-line block comment trails
inline in both formatters (`type /* c3 */ D`).

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
and [§Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent).
