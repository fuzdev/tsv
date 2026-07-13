<!-- 100 chars - stays inline (at boundary) -->
<svelte:component this={C} attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" />

<!-- 101 chars - wraps -->
<svelte:component
	this={C}
	attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
/>

<!-- With children: the content lays out block-style (both tags intact) once the attributes wrap;
     prettier dangles the delimiters instead — see prettier_variant_dangle -->
<svelte:component
	this={Comp}
	data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
>
	<span>child</span>
</svelte:component>
