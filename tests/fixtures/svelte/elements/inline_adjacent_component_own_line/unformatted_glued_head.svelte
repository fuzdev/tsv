<!--
	The adjacent separator — the whitespace-only node between two non-text siblings — before a
	COMPONENT keeps its own line. An inline element or a tag in that position flows onto the
	content line (inline_adjacent_sibling_newline_flow_prettier_divergence); a space-separated
	component sibling breaks to its own line instead, and prettier agrees.

	The cases vary only what made the container multiline — the run and its separators are the
	same in each — so the component's line does not depend on the container's cause.
-->
<p>text1
	<span>
		<b>inline1</b>
	</span>
	<Comp />
</p>

<!-- control: multiline STRUCTURALLY (a block child), not by the run's own newlines -->
<div>text1
	<span>
		<b>inline1</b>
	</span>
	<Comp />
	<div>block1</div>
</div>

<!-- control: a prose-free run, which has no fill to reflow into, reaches the same layout -->
<p><span>
		<b>inline1</b>
	</span>
	<Comp />
</p>
