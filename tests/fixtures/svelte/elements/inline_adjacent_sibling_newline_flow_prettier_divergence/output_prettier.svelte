<!--
	The sibling-newline flow rule at the ADJACENT separator — the whitespace-only node between two
	non-text siblings, which carries no prose of its own to reflow into. Svelte 5 collapses
	inter-sibling whitespace to one space, so this separator is the same document however it is
	spelled, and it flows like the text-bounded ones beside it. Without that one run reaches two
	answers: the boundaries touching a text node flow while the one between the two siblings does
	not, leaving a break in a line that fits. Covers an element pair and an expression-tag pair.
	The controls pin the rule's two edges: the same run in a container that is multiline
	STRUCTURALLY rather than by its own newlines (which already flows, so the container's cause is
	the only axis), and a prose-free run, which does not flow — there is no fill to reflow into,
	so the authored newlines are the author's only structure.
-->
<p>
	text1 text2 <span>inline1</span> <span>inline2</span> text3
</p>
<p>
	text1 text2 {expr1}
	{expr2} text3
</p>

<!-- control: multiline STRUCTURALLY (a block child), not by the run's own newlines -->
<div>
	text1 text2 <span>inline1</span> <span>inline2</span> text3
	<div>block1</div>
</div>

<!-- control: a prose-free run keeps its authored lines -->
<p>
	<span>inline1</span>
	<span>inline2</span>
</p>
