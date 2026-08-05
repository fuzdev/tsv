<script lang="ts">
	// a run of comments trailing a mapped member stays distinct, one per line
	type A = {
		[K in T]: V; // c1
		// c2
	};

	// an own-line comment after an inline block keeps its own line
	type B = {
		[K in T]: V /* c3 */;
		// c4
	};

	// a block written on its own line defers behind the line comment ahead of it,
	// keeping source order
	type C = {
		[K in T]: V; // c5
		/* c6 */
	};
</script>
