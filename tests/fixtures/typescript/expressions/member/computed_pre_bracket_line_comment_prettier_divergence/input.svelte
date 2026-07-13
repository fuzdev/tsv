<script lang="ts">
	// Line comment between the object and the `[` of a computed access: it trails
	// the object and the bracket drops to its own line.
	const a = arr // c1
		[i];

	// Two comments in the same gap stay distinct — the first trails the object,
	// the second keeps its own line.
	const b = arr // c2
		// c3
		[i];

	// Chained computed accesses break at every bracket.
	const c = arr
		[i] // c4
		[j];

	// A call in the chain: a comment inside the brackets stays inside them.
	const d =
		foo()[
			// c5
			i
		];
</script>
