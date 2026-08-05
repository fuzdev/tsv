<script lang="ts">
	// line comment in a parenthesized union type argument
	type A1 = Foo<(a | b // c
	)>;

	// call position
	const x = f<(a | b // c
	)>(y);

	// multi-argument list, forced multiline by the first argument's comment
	type A2 = Foo<
		A, // c1
		(a | b // c2
		)
	>;

	// call position, multi-argument
	const z = f<
		A, // c1
		(a | b // c2
		)
	>();
</script>
