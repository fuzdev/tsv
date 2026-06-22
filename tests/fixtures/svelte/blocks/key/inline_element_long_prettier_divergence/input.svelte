<!-- Inside an inline element a wrapped {#key} head expands like block level: its body whitespace is render-non-significant, so the body drops to its own line; the element hugs the outer boundary. Prettier keeps the whole construct inline past printWidth. -->
<span>
	{#key computeKeyValue(
		aaaaaaaaaaaaaaa,
		bbbbbbbbbbbbbbb,
		ccccccccccccccc,
		ddddddddddddddd,
		eeeeeeeeeeeeeee
	)}
		{x}
	{/key}
</span>
