<!--
	A SPACE-separated component in a multiline container, with no prose in its run to reflow into:
	the authored space is kept, exactly as an inline element keeps it. A component is inline flow
	content like a `<span>`, so the two kinds lay out identically here — the element twin beside
	each case is the parity assertion. Prettier's `isInlineElement` admits only a `RegularElement`,
	so it prints the whitespace-only node before a component as a plain `line` that breaks once the
	container is multiline: it splits the component pair while holding the element pair. A comment
	and a control-flow block keep their own lines under both formatters; what differs is only the
	space after them. With no prose in the run, the NEWLINE spelling is a fixed point of both
	formatters too (`variant_newline.svelte`) — the authored newlines are the author's only
	structure — so this fixture is about the space spelling alone.
-->
<div>
	<Comp1 />
	<Comp2 />
	<div>block1</div>
</div>
<div>
	<span>inline1</span>
	<span>inline2</span>
	<div>block1</div>
</div>

<!-- after a comment and after a control-flow block, each of which keeps its own line -->
<div>
	<span>inline1</span>
	<!-- c -->
	<Comp1 />
	{#if cond}inline2{/if}
	<Comp2 />
	<div>block1</div>
</div>
<div>
	<span>inline1</span>
	<!-- c -->
	<span>inline2</span>
	{#if cond}inline3{/if}
	<span>inline4</span>
	<div>block1</div>
</div>

<!-- at the root -->
<Comp1 />
<Comp2 />
<div>block1</div>

<!-- control: after a BLOCK element the block's own line wins, for a component as for an element -->
<div>
	<div>block1</div>
	<Comp />
	<div>block2</div>
	<span>inline1</span>
</div>
