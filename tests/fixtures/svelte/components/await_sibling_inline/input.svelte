<!--
	A component (inline) keeps a following short {#await} inline regardless of
	sibling type — text, an inline element, or {@debug} (valid inside a component,
	unlike inside a block element). Only block-element parents break; a component
	stays inline, matching prettier. A declaration ({@const} / {const} / {let} /
	{#snippet}) is the sibling kind that does break, because it takes its own
	line — see tags/declaration_own_line_prettier_divergence and
	blocks/snippet/own_line_prettier_divergence.
-->

<!-- {@debug} sibling + await -->
<Comp>{@debug x}{#await p}x{/await}</Comp>

<!-- inline-element sibling + await (breakable, but a component parent never forces) -->
<Comp><span>s</span>{#await p}x{/await}</Comp>

<!-- text sibling + await -->
<Comp>text {#await p}x{/await}</Comp>
