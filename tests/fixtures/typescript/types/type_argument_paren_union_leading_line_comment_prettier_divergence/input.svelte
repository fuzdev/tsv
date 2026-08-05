<script lang="ts">
	// leading line comment in a parenthesized union type argument
	type A1 = Foo< // c
		a | b
	>;

	// leading line comment in a parenthesized intersection type argument
	type A2 = Foo< // c
		a & b
	>;

	// parenthesized union among multiple type arguments — count changes nothing
	type A3 = Map< // c
		a | b,
		c
	>;

	// nested under an enclosing intersection
	type A4 = d &
		Foo< // c
			a | b
		>;

	// call position
	const x = f< // c
		a | b
	>(y);

	// run of two line comments — only the `<`-line one stays on the `<` line
	type A5 = Foo< // c1
		// c2
		a | b
	>;
</script>
