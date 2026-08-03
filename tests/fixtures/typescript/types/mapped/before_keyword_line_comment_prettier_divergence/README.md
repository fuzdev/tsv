# Divergence: mapped-type before-`in`/`as` line comment indents the continuation

A line comment between a mapped type's key name and its `in` keyword
(`[K // c⏎in T]`), or between the constraint and its `as` clause
(`[K in T // c⏎as U]`). A `//` runs to end-of-line, so the keyword and its
operand cannot stay on the comment's line — inlining would swallow them
(content loss). tsv keeps the comment where the author wrote it — trailing the
name / constraint — and drops the whole `in T` / `as U` tail to a continuation
line **indented one level** (uniform forced-continuation indent). Prettier
instead **expands the brackets** and floats the comment *past* the keyword to
the end of the binding.

```ts
// tsv (preserve + continuation)   // prettier (expand + float past the keyword)
type A = {                         type A = {
	[K // c                           [
		in T]: V;                        K in T // c
};                                    ]: V;
                                   };
```

Prettier's float is **information-destructive on a run**: `K // c1⏎// c2⏎in T`
merges both comments onto one line **in reverse order** (`K in // c2 // c1⏎T`,
the second `//` becoming text), then collapses to `K in T // c2 // c1` on the
next pass — the two-pass chain `audit_signature.txt` pins. tsv keeps each
comment distinct, in order, at its authored position.

The **own-line** authoring (`K⏎// c⏎in T`) pulls up to trail the name and
reaches input under tsv in one pass — own-line-ness is authoring signal for a
leading position, not a trailing one. It carries no `unformatted_ours_own_line`
pin because its prettier chain is not expressible: prettier takes *two* passes
from it (`K in // c⏎T`, then `K in T // c`) and lands on `output_prettier`, a
target no `prettier_intermediate*_*` marker accepts (N7 → `input`, N7b →
`variant_*`, N7c → `divergent_variant_*`). The **flush** continuation
(`unformatted_ours_flush.svelte`) normalizes to input in one pass and is pinned.
A **single-line block** in either gap stays inline in both
formatters (`[K /* c */ in T]`, [mapped_bracket_comment](../../mapped_bracket_comment/)),
and a **multiline** block the author broke after glues to the keyword in both —
the broke-after continuation rule is scoped to value-separator gaps, and
prettier does not distinguish the broke-after authoring here either.

The mapped-type face of the
[type-parameter pre-keyword gaps](../../comments/type_param_before_extends_line_comment_prettier_divergence/)
(name→`extends`, before-`=`), which take the same one-level continuation. The
*after*-keyword gaps (`[K in // c⏎T]`) are
[keyword_line_comment](../keyword_line_comment_prettier_divergence/), and the
`]`→`:` gap is
[mapped_bracket_colon_line_comment](../../mapped_bracket_colon_line_comment_prettier_divergence/).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Uniform Forced-Continuation Indent, §Comment Position Philosophy and
§Comment relocation.
