<script lang="ts">
	// A zero-argument call is a "poorly breakable" chain — the `=` breaks — unless its
	// type arguments give it an internal break point of their own.
	// Prettier ref: isCallExpressionWithComplexTypeArguments (assignment.js:422)

	// Line comment in the type arguments: forces the `<…>` open, so the `=` hugs
	const aaaaaaaaaaaaaaaaaaaaaaaaaaaa = createSomethingWithALongName<
		A // c
	>();

	// Own-line block comment: the same forced expansion
	const bbbbbbbbbbbbbbbbbbbbbbbbbbbb = createSomethingWithALongName<
		/* c */
		A
	>();

	// Union type argument: complex unconditionally, no comment needed
	const cccccccccccccccccccccccccccc = createSomethingWithALongName<
		AaaaaaaaaaaaaaaaaaaaaaaaLong | BbbbbbbbbbbbbbbbbbbbbbbbLong
	>();

	// Plain type reference: NOT complex — the `=` breaks instead
	const dddddddddddddddddddddddddddd =
		createSomethingWithALongName<AaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaLong>();
</script>
