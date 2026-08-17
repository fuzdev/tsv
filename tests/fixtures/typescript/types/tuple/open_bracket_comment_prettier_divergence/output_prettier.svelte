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

	// an author blank BELOW the pulled comment survives — the blank is authorship, not
	// the container's leading gap (the blank ABOVE one stays erased, against the delimiter)
	type D = [
		// c4

		string
	];
</script>
