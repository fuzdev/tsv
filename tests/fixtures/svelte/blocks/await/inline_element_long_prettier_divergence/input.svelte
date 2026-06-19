<!-- Inside an inline element a wrapped {#await} head dangles and the whole block expands like block level: the then-body and {/await} drop to their own lines (body whitespace is render-non-significant); the element hugs the outer boundary. Prettier keeps the whole construct inline past printWidth. -->
<span
	>{#await getData(
		aaaaaaaaaaaaaaa,
		bbbbbbbbbbbbbbb,
		ccccccccccccccc,
		ddddddddddddddd,
		eeeeeeeeeeeeeee,
	) then r}
		{r}
	{/await}</span
>
