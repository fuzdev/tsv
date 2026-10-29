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

	// A call in the chain: a comment inside the brackets stays inside them, and the
	// broken lookup hangs below the `=`, as prettier's does.
	const d =
		foo()[
			// c5
			i
		];

	// A comment inside the first brackets and one before the next `[`: the chain breaks
	// at every lookup, each comment where it was written.
	q.r = arr
		[
			// c6
			y.p
		] // c7
		?.[y]
		.c;

	// The next index led by a `//` inside a stripped shell lays out as the comment
	// written in its brackets.
	q = a
		[i] // c8
		?.[
			// c9
			y.p
		];
</script>
