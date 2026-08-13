<!--
	Air on ONE boundary. Where both content boundaries carry a newline every container kind
	preserves the air alike (inline_boundary_air); asked of a single boundary each kind answers
	by its boundary rule alone:

	- an INLINE element is both-or-neither — one boundary is not a request for air, so it collapses
	- a COMPONENT is both-or-neither too
	- a BLOCK expands on its leading boundary alone; a trailing boundary alone collapses

	Where the newline LANDS is not part of the question: a leading boundary in front of text sits
	inside that text node's edge run, in front of an element it is a whitespace-only node — both
	are the same boundary and answer alike.
-->

<!-- inline, leading only: collapses -->
<div>
	<span>text1 <b>x</b> text2</span>
</div>

<!-- inline, trailing only: collapses -->
<div>
	<span>text1 <b>x</b> text2</span>
</div>

<!-- block, leading only: expands -->
<div>
	<p>
		text1 <b>x</b> text2
	</p>
</div>

<!-- block, trailing only: collapses -->
<div>
	<p>text1 <b>x</b> text2</p>
</div>

<!-- component, leading only, content starts with text: collapses -->
<div>
	<Comp>text1 <b>x</b> text2</Comp>
</div>

<!-- component, leading only, content starts with an ELEMENT: collapses -->
<div>
	<Comp><b>x</b> text2</Comp>
</div>

<!-- component, trailing only: collapses -->
<div>
	<Comp>text1 <b>x</b> text2</Comp>
</div>
