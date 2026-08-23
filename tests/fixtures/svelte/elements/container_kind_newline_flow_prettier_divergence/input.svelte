<!--
	The sibling-newline flow rule across CONTAINER KINDS. The rule is asked of the RUN — is there
	prose for a fill to pack? — never of the container holding it, so an inline element, a block
	element and a component lay one run out identically. No other fixture in this family varies
	the container to say so: nearly all put the run in a BLOCK container, and the one inline
	container that carries a flowing run reaches multiline past a 99-char attribute line
	(inline_attrs_multiline_content), where the kind is confounded with the attribute wrap. An
	inline container reaches the multiline form by a different boundary rule (both-or-neither,
	rather than the block's leading boundary alone), so it is where a container-keyed rule would
	show. The `svelte:*` pair covers the fourth kind on both arities — a SpecialElement projects
	onto the same analysis a regular element does, and that projection is what the pair pins.
	Both boundaries carry air in every case, so each element stays multiline and only the interior
	separator flows. The control is a prose-free run in an inline container: with no fill to reflow
	into, its authored lines are the author's only structure and both formatters keep them.
-->

<!-- inline element -->
<button>
	<Comp1 /> text1 {expr1}
</button>

<label>
	<Comp1 /> text1 {expr1}
</label>

<!-- block element: the same run, the same answer -->
<p>
	<Comp1 /> text1 {expr1}
</p>

<!-- component: the same -->
<Comp2>
	<Comp1 /> text1 {expr1}
</Comp2>

<!-- special element, inline-classified: the `svelte:*` projection answers alike -->
<svelte:element this={tag}>
	<Comp1 /> text1 {expr1}
</svelte:element>

<!-- special element, block-classified: the same, on the other boundary arity -->
<svelte:head>
	<Comp1 /> text1 {expr1}
</svelte:head>

<!-- control: a prose-free run has no fill, so its authored lines are structure and stay -->
<button>
	<Comp1 />
	{expr1}
</button>
