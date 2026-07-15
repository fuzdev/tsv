<script lang="ts">
	// Trailing arg is a TS-wrapped collection. Prettier's couldExpandArg recurses
	// through `as`/`satisfies`/`<T>`, so the collection hugs the call and expands
	// internally (expand-last) while the head arg stays inline.

	// `as` wrapping a trailing object
	fn(head, {
		prop1: 1,
		prop2: 2
	} as T);

	// `satisfies` wrapping a trailing object
	fn(head, {
		prop1: 1,
		prop2: 2
	} satisfies T);

	// angle-bracket assertion wrapping a trailing object
	fn(head, <T>{
		prop1: 1,
		prop2: 2
	});

	// `as` wrapping a trailing array (elements exceed print width, so the array breaks)
	fn(head, [
		elementaaaaaaaa,
		elementbbbbbbbb,
		elementcccccccc,
		elementdddddddd,
		elementeeee
	] as const);
</script>
