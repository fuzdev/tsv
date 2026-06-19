<!-- A component (inline) keeps a following short {#await}/{#snippet} inline regardless of
sibling type — text, an inline element, or {@const}/{@debug} (valid inside a component,
unlike inside a block element). Only block-element parents break; a component stays inline,
matching prettier. -->

<!-- {@const} sibling + await -->
<Comp>{@const a = 1}{#await p}x{/await}</Comp>

<!-- {@debug} sibling + await -->
<Comp>{@debug x}{#await p}x{/await}</Comp>

<!-- inline-element sibling + await (breakable, but a component parent never forces) -->
<Comp><span>s</span>{#await p}x{/await}</Comp>

<!-- text sibling + await -->
<Comp>text {#await p}x{/await}</Comp>

<!-- {@const} sibling + snippet -->
<Comp>{@const a = 1}{#snippet f()}x{/snippet}</Comp>
