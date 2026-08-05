<script lang="ts">
	// A comment after the LAST argument survives every layout that force-expands the
	// argument list, in the plain call and in `new`.

	// function composition (an argument is a call with a callback): the block comment
	// trails the last argument
	foo(
		arr.map((x) => x),
		b /* c */
	);

	// same layout, comment on its own line: it stays on its own line before the `)`
	foo(
		arr.map((x) => x),
		b
		/* c */
	);

	// multiline content (a legacy line continuation) forces the same expansion
	foo(
		'aa\
bb',
		b /* c */
	);

	// same path, line comment trailing the last argument
	foo(
		'aa\
bb',
		b // c
	);

	// `new` reaches the multiline-content expansion too
	new Foo(
		'aa\
bb',
		b /* c */
	);

	// expand-first falls back to one argument per line when a tail argument breaks
	foo(
		() => {
			doThing();
		},
		bar(
			// i
			a
		) /* c */
	);

	// a blank line between arguments forces the same expansion in `new`
	new Foo(
		a,

		b /* c */
	);
</script>
