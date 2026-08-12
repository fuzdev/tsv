<!--
	Air on ONE boundary. Where both boundaries carry a newline every container kind preserves the
	air alike (inline_boundary_air); asked of one boundary the kinds answer differently, and the
	arity is the whole rule:

	- an INLINE element is both-or-neither — one boundary is not a request for air, so it collapses
	- a BLOCK expands on its leading boundary alone
	- a COMPONENT is both-or-neither too, but any newline inside a CONTENT TEXT counts for it, so a
	  leading boundary that lands in a text node expands it while the same boundary in front of an
	  element does not

	The last pair is the one that looks like an exception and is not: the newline is in a different
	NODE, not in a different position.
-->

<!-- inline, leading only: collapses -->
<div>
	<span>
		text1 <b>x</b> text2</span>
</div>

<!-- inline, trailing only: collapses -->
<div>
	<span>text1 <b>x</b> text2
	</span>
</div>

<!-- block, leading only: expands -->
<div>
	<p>
		text1 <b>x</b> text2</p>
</div>

<!-- component, leading only, content STARTS WITH TEXT: expands -->
<div>
	<Comp>
		text1 <b>x</b> text2</Comp>
</div>

<!-- component, leading only, content starts with an ELEMENT: collapses -->
<div>
	<Comp>
		<b>x</b> text2</Comp>
</div>
