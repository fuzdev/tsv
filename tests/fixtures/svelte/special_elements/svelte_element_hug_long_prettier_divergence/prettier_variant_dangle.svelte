<!-- 100 chars - stays inline (at boundary) -->
<svelte:element this="span" class="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" />

<!-- 101 chars self-closing - wraps all attrs -->
<svelte:element
	this="span"
	class="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
/>

<!-- 101 chars with text children: the content lays out block-style (both tags intact);
     prettier dangles the delimiters instead — see prettier_variant_dangle -->
<svelte:element this="span" class="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
	>text</svelte:element
>
