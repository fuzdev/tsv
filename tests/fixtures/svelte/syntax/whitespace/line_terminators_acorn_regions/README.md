# line_terminators_acorn_regions

One `<LS>` (or `<PS>`) in each island Svelte hands to acorn, each in a region the
printer copies verbatim so the document stays a fixed point under both formatters.
Svelte prepares a different source for every island, so this is one case per
preparation:

| island | source acorn receives |
| --- | --- |
| `<script>` (`read_script`) | prefix blanked with `replace(/[^\n]/g, ' ')` + content |
| `{expr}`, an attribute value (`read_expression`) | the **raw** template |
| `{@const}`'s init (`read_expression`) | the raw template |
| a pattern binding — `{@const}`'s id, a destructured `{#each … as { … }}` (`read_pattern`) | the raw template up to the pattern's end + ` = 1` |
| a trailing `: T` (`read_type_annotation`) | the raw prefix + `_ as ` over the units before the colon + raw rest |
| `{#snippet}` parameters | the raw template up to the parameters' end + ` => {}` |

The fixture pins the spans: each terminator is three UTF-8 bytes and one UTF-16 code
unit, in every island.

Svelte's own `loc` counts the ECMAScript terminators for acorn-parsed nodes — in a
document that holds one, every terminator ahead of the node in the source acorn
received — and `\n` alone everywhere else. tsv's
does not reproduce that: a Svelte document counts `\n` alone for every `loc` in it,
so none of these terminators opens a line. That is graded by
[`tests/loc_definition.rs`](../../../../../loc_definition.rs), whose fixture walk
reads this input, and the difference from Svelte's wire is cataloged in
[conformance_svelte.md](../../../../../../docs/conformance_svelte.md).

A lone `<CR>` belongs to the same class but cannot be a fixture input — every
parse-then-format entry point folds it to `<LF>` before parsing, so such a
document is not the fixed point F1 requires; `tests/loc_definition.rs` holds that
case too.

Sibling fixtures: [line_terminators](../line_terminators/) (output folding) and
[line_terminators_comment_dedent](../line_terminators_comment_dedent/) (the
comment `value`).
