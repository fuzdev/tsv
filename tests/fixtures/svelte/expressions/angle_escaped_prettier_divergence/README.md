# angle_escaped_prettier_divergence

tsv: two `{expr}` tags separated by a single authored newline, inside a run that holds prose,
flow onto one content line. Prettier: keeps each tag on its authored line.

## Reason

**Design choice — the sibling-newline flow rule, at a separator between two tags.** Svelte 5
collapses an inter-sibling whitespace run to one whitespace, so a space and a newline between
two siblings render identically; the newline's *spelling* carries no signal and the run reflows
per width. That is the same convergence
[inline_sibling_newline_flow](../../elements/inline_sibling_newline_flow_prettier_divergence/)
pins for a text↔sibling boundary — here the separator is a whitespace-only node standing
*between two non-text siblings*, which is the shape that used to keep its structural break.

The tag case is where the two formatters part. Prettier trims a space-only boundary before an
inline **element** to a collapsible break (so `</span>` ` ` `<span>` reflows and both formatters
agree — see `script/escapes`, `style/escapes`), but before a **tag** it emits a plain `line`
into the child list, which resolves all-or-nothing with the parent group. Since a multiline
fragment's parent group is always broken, every such boundary breaks. tsv instead defers the
boundary to the tag itself, giving it the same per-width `group([line, tag])` an inline element
gets — so the whole run reflows as one rather than flowing the single boundary owned by a
content text's fill and hard-breaking the rest.

`{'<'}div{'>'}` and `{'<'}code{'>'}{'<'}/code{'>'}` are one document under Svelte 5 whether
separated by a space or a newline, so tsv converges them; prettier holds a stable form for each.

## Cases

The one shape that diverges, and nothing else: two escaped-bracket tags on adjacent lines, in a
run whose glued `div` / `code` words are the prose the rule needs. The comment is load-bearing —
it bounds the run above, so the pair is the whole flowing run.

The rest of the escaped-angle-bracket matrix lives in the sibling
[angle_escaped](../angle_escaped/), where both formatters agree: those cases either have no
tag↔tag boundary at all, or their tags are glued (`{'<'}p{'>'}paragraph{'<'}/p{'>'}`), and a
glued boundary is never split — breaking there would inject a rendered space. Keeping them out
of this fixture is the point: a `_prettier_divergence` should assert the divergence, not carry
agreement cases whose regressions would read as noise in a divergence diff.

## Controls — what does NOT flow

The rule's own controls (comment neighbour, blank line, prose-free run, control-flow block) are
pinned by
[inline_sibling_newline_flow](../../elements/inline_sibling_newline_flow_prettier_divergence/),
which owns them. The control that belongs *here* is the sibling fixture above: the same escape
syntax, one boundary shape away, converging with prettier.

The run here sits in the **root fragment**, which is why the rule applies at all. Inside an
element whose content went multiline *because of* these same newlines, collapsing them would
delete the break that chose the multiline layout, and the next pass would split the run back
apart — so the rule stands down there and the tags keep their authored lines. That boundary is
pinned by
[inline_content_spaced_tags_tail_long](../../elements/inline_content_spaced_tags_tail_long/).

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
