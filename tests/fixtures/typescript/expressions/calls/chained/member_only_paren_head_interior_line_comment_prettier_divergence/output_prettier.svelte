<script lang="ts">
	// A parenthesized head (a cast) with a line comment before the member: the
	// chain breaks at the member and the comment stays on the head's line.
	const a = (
		b as T
	) // c
	.d;

	// The statement's own trailing comment keeps its place on the member's line —
	// merging the two would turn the second `//` into text of the first.
	const e = (
		f as T
	) // c1
	.g; // c2

	// An `await` head behaves the same way.
	async function fn() {
		return (
			await p
		) // c
		.h;
	}
</script>
