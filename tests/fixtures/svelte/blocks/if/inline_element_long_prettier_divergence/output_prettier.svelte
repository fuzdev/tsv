<!-- Inside an inline element a wrapped {#if} head dangles and the whole block expands like block level: consequent, {:else}, and {/if} drop to their own lines (body whitespace is render-non-significant); the element hugs the outer boundary. Prettier keeps the whole construct inline past printWidth. -->
<span>
	{#if aaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbb && cccccccccccccccc && dddddddddddddddd && eeeeeeeeeeeeeeee}
		{x}
	{:else}
		{y}
	{/if}
</span>
