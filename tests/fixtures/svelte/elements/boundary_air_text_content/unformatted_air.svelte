<!--
	Authored boundary air is honored per fragment family, and the two families part on
	TEXT-ONLY content: a block body honors it, while an element's and a component's
	text-only content collapses and width alone decides.

	Both formatters agree on every case here, so the split is inherited rather than chosen,
	and neither records an argument for it. This fixture pins the behavior so that changing
	it would be a decision rather than an accident.

	The air itself is the shared rule (inline_boundary_air), and its ARITY is pinned next
	door (boundary_air_one_sided) for the element kinds. What this one holds fixed is the
	CONTENT KIND against the family, the axis neither of those varies.

	The controls carry the shape they exclude — the same containers with a non-text child,
	where every family honors the air — so "a block body holds air" cannot be satisfied by a
	rule that never reads the content, and "an element collapses" cannot be satisfied by one
	that never reads the family.
-->

<!-- block body, text-only content: the air is honored -->
{#if cond}
	text1 text2
{/if}

<!-- and the hugged authoring is its own fixed point, not a form that expands -->
{#if cond}text1 text2{/if}

<!-- block element, text-only content: the air collapses, width alone decides -->
<div>
	text1 text2
</div>

<!-- component parity, same content -->
<Comp>
	text1 text2
</Comp>

<!-- control: a NON-text child, where the element kinds honor the air after all -->
<div>
	<b>x</b>
</div>

<!-- control: component parity for the same -->
<Comp>
	<b>x</b>
</Comp>

<!-- control: the block body honors it there too, so the family is not simply "always holds" -->
{#if cond}
	<b>x</b>
{/if}
