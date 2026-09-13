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

	// a block glued behind an own-line block of a deferred run stays behind it: the
	// run keeps its order and its lines
	type D = {
		[K in T]: V; // c7
		/* c8 *//* c9 */
	};

	// a multi-line block glued there answers the same way
	type E = {
		[K in T]: V; // c10
		/* c11 *//* c12
c13 */
	};
</script>
