<script lang="ts">
	// Two comments before `.baz`: the first trails `.bar`, the second on its own
	// line; the chain breaks at every member.
	const a =
		// c2
		foo
			.bar // c1
		.baz;

	// A single own-line comment before a member.
	const b =
		// c1
		foo.bar.baz;

	// A lookup with a comment in its brackets takes the same path: the gap comment after it
	// keeps its own line.
	const c =
		// c4
		arr // c3
		[i].baz;

	// A block in the brackets, the same.
	const d =
		// c6
		arr[/* c5 */ i].baz;
</script>
