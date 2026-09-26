<!--
	A directive glued to the text in front of a frozen block element stays glued, and the boundary
	after the element is the one it gets unfrozen: whitespace before an inline follower is a line
	break.
-->

<!-- at the root, text after -->
text1<!-- prettier-ignore --><div    data-attr="value"   >block1</div> text2

<!-- inside an element, an inline element after -->
<div>
	text3<!-- prettier-ignore --><p    data-attr="value"   >block2</p> <span>inline1</span>
</div>

<!-- inside a block body, a component after -->
{#if cond}
	text4<!-- prettier-ignore --><p    data-attr="value"   >block3</p> <Comp />
{/if}
