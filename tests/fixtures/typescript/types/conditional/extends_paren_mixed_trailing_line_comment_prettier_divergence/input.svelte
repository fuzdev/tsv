<script lang="ts">
	// Mixed: a block then a line comment leading the extends-type. The block trails
	// `extends` inline, the line comment forces the extends-type onto its own line.
	type T1 = A extends /* b */ // c
		B
		? X
		: Y;

	// Trailing: a line comment leading, a block trailing the extends-type. The line
	// hangs the type; the trailing block stays inline at this type position.
	type T2 = A extends // c
		B /* t */
		? X
		: Y;

	// A run the author GLUED — a multi-line block ahead of a single-line one — and then broke
	// after. The run's break is forced by the multi-line block, so the extends-type hangs
	// below the whole run exactly as it does under the `//` above.
	type T3 = A extends /* a
b */ /* c */
		B
		? X
		: Y;
</script>
