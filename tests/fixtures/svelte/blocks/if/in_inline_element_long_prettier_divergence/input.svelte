<!-- 100 chars - stays on one line (at boundary) -->
<span
	>{#if typeof content_value === 'string'}{content_value}{:else}{@render content_value()}{/if}</span
>

<!-- 101 chars - prettier keeps on one line (exceeds print width) -->
<span
	>{#if typeof content_value1 ===
		'string'}{content_value}{:else}{@render content_value()}{/if}</span
>
