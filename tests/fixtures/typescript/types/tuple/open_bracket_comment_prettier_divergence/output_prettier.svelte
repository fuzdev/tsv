<script lang="ts">
	// line comment trailing tuple `[` - single element
	type A = [
		// c1
		string
	];

	// line comment trailing tuple `[` - multiple elements
	type B = [
		// c2
		string,
		number
	];

	// block comment trailing `[`, then own-line leading comment
	type C = [
		/* bracket */
		// lead
		boolean
	];
</script>
