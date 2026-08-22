# line_terminators_acorn_regions

Every `loc` in the Svelte wire is one of **two** line counts, and which one a node
gets is decided by the acorn parse it came from.

Svelte's own positions (`locate-character`) open a line at `\n` and nothing else.
Everything acorn parses carries acorn's, which is the ECMAScript class — `\n`,
`\r`, `\r\n`, `<LS>`, `<PS>` — and acorn seeds that counter **once per parse**,
over whatever prefix Svelte prepared for it. Svelte prepares a different one at
every island, so this fixture is one case per preparation, with a `<LS>` (or a
`<PS>`) in a region the printer copies verbatim so the document stays a fixed
point under both formatters.

| island | source acorn receives | effect here |
| --- | --- | --- |
| `<script>` (`read_script`) | prefix blanked with `replace(/[^\n]/g, ' ')` + content | the `module` script's own nodes shift |
| `{expr}`, an attribute value (`read_expression`) | the **raw** template | every one shifts, including for terminators in earlier islands |
| `{@const}`'s init (`read_expression`) | the raw template | shifts |
| a pattern binding — `{@const}`'s id, a destructured `{#each … as { … }}` (`read_pattern`) | blanked prefix + `(pattern = 1)` | its own interior terminator shifts its nodes |
| a trailing `: T` (`read_type_annotation`) | blanked prefix + `_ as ` + raw rest | same, and it is a *second* parse with its own seed |
| `{#snippet}` parameters | prefix `replace(/\S/g, ' ')` — whitespace survives | behaves as the raw template does |

**The instance `<script>` is the null control.** Its body sits after the module
script's `<LS>`, and its `loc` must not move: `read_script` blanks the whole
prefix, that earlier script included, so the terminator never reached its parse.
Routing acorn's islands to a plain ECMAScript-rule line table — the obvious fix —
breaks exactly this node.

`<p>text1<LS>text2{b}</p>` is the second one. acorn seeds `lineStart` with
`lastIndexOf("\n", startPos - 1)` and then jumps straight to `startPos`, so a
terminator between that LF and the tag is counted by neither half: its `{b}`
keeps the column the LF line start gives it while the *line* still carries every
terminator from earlier in the document.

A lone `<CR>` belongs to the same class but cannot be a fixture input — every
parse-then-format entry point folds it to `<LF>` before parsing, so such a
document is not the fixed point F1 requires. That half is pinned by
[`tests/acorn_loc_line_terminators.rs`](../../../../../acorn_loc_line_terminators.rs),
with `\r\n` as its null control (one ECMAScript break holding one LF, so the two
classes never disagree over it).

Sibling fixtures: [line_terminators](../line_terminators/) (output folding) and
[line_terminators_comment_dedent](../line_terminators_comment_dedent/) (the
comment `value`).
