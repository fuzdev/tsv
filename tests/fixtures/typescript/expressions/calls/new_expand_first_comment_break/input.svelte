<script lang="ts">
	// `new` expand-first breaks all args for comments its inline tail can't carry,
	// matching the plain-call path and prettier: a line comment leading the second
	// arg, or a block comment trailing the first arg before the comma. (An inline
	// block comment leading the second arg is instead carried inline — see
	// expand_first_leading_block_comment.)

	// line comment leading the second arg
	new Foo(
		() => {
			doThing();
		}, // c
		x
	);

	// block comment trailing the first arg, before the comma
	new Foo(
		() => {
			doThing();
		} /* c */,
		x
	);
</script>
