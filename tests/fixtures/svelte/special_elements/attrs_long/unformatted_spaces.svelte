<!-- 100 chars - stays inline (at boundary) -->
<svelte:component
	this={C}
	attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
/>

<!-- 101 chars - wraps -->
<svelte:component   this={C}   attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"   />

<!-- With children - hug mode wrapping -->
<svelte:component   this={Comp}   data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"   ><span>child</span></svelte:component>
