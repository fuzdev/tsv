<!-- Short svelte:component - no wrap (63 chars) -->
<svelte:component this={Comp} prop={value} data-attr="short" />

<!-- Long svelte:component - wraps (102 chars, exceeds 100) -->
<svelte:component
	this={Comp}
	data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
/>

<!-- Long svelte:component with children - wraps (101 chars, exceeds 100) -->
<svelte:component
	this={Comp}
	data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
>
	<span>child</span>
</svelte:component>
