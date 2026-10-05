<script lang="ts">
	// A numeric literal ends at its own last digit: a `.` behind a complete literal is a
	// member access on it, which no type continues with, so the `<` is the comparison
	// operator. Each line is the chain the bare authoring in the `unformatted_bare_literal`
	// variant reads as.
	const a1 = (a < b[c]) | ((1).x > e);
	const a2 = (a < b[c]) | ((1.5).x > e);
	const a3 = (a < b[c]) | ((0.5).x > e);
	const a4 = (f < A[K]) | ((1).x > e);
	const a5 = a < (1).x > e;
	const a6 = (a < b[(1).x]) | (c > e);
	const a7 = a < (1e3).x > e;
	const a8 = a < (0x1).x > e;
	const a9 = a < 1n.x > e;
</script>
