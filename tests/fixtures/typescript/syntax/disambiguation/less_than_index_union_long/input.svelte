<script lang="ts">
	// An argument list that fits at 100 columns stays flat; at 101 the call's arguments
	// break and the type-argument list stays flat.
	const a1 = fn<A[K] | BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB>(x);
	const a2 = fn<A[K] | BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB>(
		x
	);

	// Before a template, with no arguments to break, the type-argument list itself breaks
	// at 101 columns.
	const b1 = fn<A[K] | BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB>`t`;
	const b2 = fn<
		A[K] | BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB
	>`t`;
</script>
