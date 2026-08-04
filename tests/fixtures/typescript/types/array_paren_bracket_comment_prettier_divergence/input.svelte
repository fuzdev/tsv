<script lang="ts">
	// A block comment inside the brackets of a parenthesized element stays inside them.
	type A = (X & Y)[/* c */];

	// A line comment inside the brackets breaks them open, like an empty tuple type.
	type B = (X & Y)[
		// c
	];

	// A hugged union element takes the same route as any other parenthesized element.
	type C = (T | null)[/* c */];

	// Each suffix in a chain of brackets keeps its own comment.
	type D = (X & Y)[/* c1 */][/* c2 */];

	// The gap and the brackets stay two positions; prettier collapses them onto one.
	type E = (X & Y) /* c1 */[/* c2 */];
</script>
