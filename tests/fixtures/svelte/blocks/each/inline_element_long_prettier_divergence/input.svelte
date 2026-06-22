<!-- Inside an inline element a wrapped {#each} head expands like block level: its body whitespace is render-non-significant, so the body drops to its own line; the element hugs the outer boundary. Prettier keeps the whole construct inline past printWidth. -->
<span>
	{#each getItems(
		aaaaaaaaaaaaaaa,
		bbbbbbbbbbbbbbb,
		ccccccccccccccc,
		ddddddddddddddd,
		eeeeeeeeeeeeeee
	) as item}
		{item}
	{/each}
</span>
