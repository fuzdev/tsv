<!--
	The sibling-newline flow rule at the ADJACENT separator before a COMPONENT — the whitespace-only
	node between two components, or between an element and a component in either order. A component
	is inline flow content exactly as a `<span>` is, so when the run holds prose that separator is
	a spelling of a space and the run packs per width, whichever kind sits on either side of it.
	The element pair beside the first case is the parity assertion: the two pairs lay out
	identically. Prettier instead breaks before a component whenever the container is multiline —
	from the space spelling as well as the newline one — while the text-adjacent boundaries hug, so
	one run reaches two answers in a line that fits. The controls pin the rule's edges: the same run
	in a container that is multiline STRUCTURALLY (a block child) rather than by its own newlines,
	and a prose-free run, which does not flow — with no fill to reflow into, the authored newlines
	are the author's only structure.
-->
<p>
	text1 <Comp1 />
	<Comp2 /> text2
</p>
<p>
	text1 <span>inline1</span> <span>inline2</span> text2
</p>

<!-- kind order: an element before a component, and a component before an element -->
<p>
	text1 <span>inline1</span>
	<Comp /> text2
</p>
<p>
	text1 <Comp /> <span>inline1</span> text2
</p>

<!-- components with children -->
<p>
	text1 <Comp1>inline1</Comp1>
	<Comp2>inline2</Comp2> text2
</p>

<!-- prose only at the END of the run: every separator in the run flows, not just the last -->
<p>
	<Comp1 />
	<Comp2 />
	<Comp3 /> text
</p>

<!-- control: multiline STRUCTURALLY (a block child), not by the run's own newlines -->
<div>
	text1 <Comp1 />
	<Comp2 /> text2
	<div>block1</div>
</div>

<!-- control: a prose-free run keeps its authored lines -->
<p>
	<Comp1 />
	<Comp2 />
</p>
