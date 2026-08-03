# Divergence: named-tuple label→`:` line comment indents the continuation

A line comment between a named tuple member's label and its `:` (`[b // c⏎: T]`).
A `//` runs to end-of-line, so the `: T` cannot stay on the comment's line —
inlining would swallow it. tsv keeps the comment where the author wrote it —
trailing the label — and drops the `: type` to a continuation line **indented one
level** (uniform forced-continuation indent). Prettier **relocates** the comment
across the `:` and past the element type, to the end of the member.

```ts
// tsv (preserve + continuation)   // prettier (relocate to end-of-member)
type A = [                         type A = [
	b // c                            b: string // c
		: string                    ];
];
```

The optional `?` marker's gap (`[b? // c⏎: T]`) and a rest member's label gap
(`[...r // c⏎: T[]]`) take the same continuation — the member's own label→`?`
gap already did.

Prettier's relocation is **information-destructive on a run**: `b // c1⏎// c2⏎: T`
merges both comments onto one line **in reverse order** (`b: // c2 // c1⏎T`, the
second `//` becoming text), then collapses to `b: T // c2 // c1` on the next pass
— the two-pass chain `audit_signature.txt` pins. tsv keeps each comment distinct,
in order, at its authored position.

The **own-line** authoring (`b⏎// c⏎: T`) pulls up to trail the label and reaches
input under tsv in one pass — own-line-ness is authoring signal for a leading
position, not a trailing one. It carries no `unformatted_ours_own_line` pin
because its prettier chain is not expressible: prettier takes *two* passes from
it (`b: // c⏎string`, then `b: string // c`) and lands on `output_prettier`, a
target no `prettier_intermediate*_*` marker accepts (N7 → `input`, N7b →
`variant_*`, N7c → `divergent_variant_*`). The **flush** continuation
(`unformatted_ours_flush.svelte`, prettier's indent for the sibling before-`:`
sites) normalizes to input in one pass under both formatters and is pinned. A **multiline** block the
author broke after stays glued to the `:` in both formatters — the broke-after
continuation rule is scoped to value-separator gaps, and prettier does not
distinguish the broke-after authoring here either (as at the [switch case
head→`:`](../../../statements/switch/case_before_colon_own_line_block_comment_prettier_divergence/)
gap); the single-line block sibling is
[tuple_label_comment](../../tuple_label_comment/).

The named-tuple face of the cross-construct
[before-`:` continuation indent](../../type_members/index_signature_key_colon_line_comment_prettier_divergence/)
(index signatures, property signatures, class properties, variable bindings,
function parameters). The member's *after*-`:` gap is
[member_line_comment](../member_line_comment_prettier_divergence/).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Uniform Forced-Continuation Indent, §Comment Position Philosophy and
§Comment relocation.
