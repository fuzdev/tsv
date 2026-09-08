<script lang="ts">
	// a hugging union return after a `=>`-trailing line comment
	type A = () => // c1
		Map<string, number> | null;
	type B = () => // c2
		{ a: 1 } | null;

	// a non-hugging union hangs the comment with its members
	type C = () =>
		// c3
		A | B;
</script>
