<!--
	The boundary after a frozen block element is the printer's, and it is the one the element
	gets unfrozen: whitespace between it and an inline follower is a line break.
-->

<!-- text after -->
<!-- prettier-ignore -->
<div    data-attr="value"   >block1</div>text1

<!-- an inline element after, inside an element -->
<div>
	<!-- prettier-ignore -->
	<p    data-attr="value"   >block2</p><span>inline1</span>
</div>

<!-- a component after, inside a block body -->
{#if cond}
	<!-- prettier-ignore -->
	<p    data-attr="value"   >block3</p><Comp />
{/if}
