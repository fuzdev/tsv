# Divergence: named-import-specifier rename `as` gaps (preserve, one indent level)

A *line* comment in a renamed specifier's `imported`→`as` gap (`a //c⏎as b`) or `as`→`local` gap
(`e as //c4⏎f`). tsv keeps each where the author wrote it and drops the tail (`as <local>`, or the
`local` after `as`) to a continuation line at **one** indent level; prettier **relocates** every
such comment to lead the whole specifier and stacks them flush before it.

```ts
// tsv (preserve)          // prettier (relocate to lead the specifier)
import {                   import {
	a //c                     //c
		as b                  a as b
} from 'x';                } from 'x';
```

This is the named-specifier analog of the sanctioned namespace `*`→`as` gap
(`import * //c⏎as ns`): the same shared renamed-specifier printer (`build_renamed_specifier_doc`)
routes both `as` gaps through the shared header-gap continuation helpers, so a line comment can't
swallow the `as` or the renamed binding. The before-`as` `//` case is a **content-loss** fix —
tsv previously emitted `a //c as b` on one line, dropping `as b` into the comment (the local
binding silently changed `b` → `a`, and the output no longer reparsed). A same-line block comment
trails inline in both formatters.

The "one indent level, not a staircase" rendering is the same rule the multi-gap
[export_as_namespace_line_comment](../../exports/export_as_namespace_line_comment_prettier_divergence/)
pins for the header case.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation)
and [§Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent).
