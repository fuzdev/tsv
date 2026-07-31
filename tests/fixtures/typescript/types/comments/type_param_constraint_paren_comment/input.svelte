<script lang="ts">
	// A line comment after `extends` hangs the parenthesized conditional
	// constraint (clarity parens kept).
	function fn<
		T extends // c
			(A extends B ? C : D)
	>(): void {}

	// A same-line block comment stays inline before the parens.
	type E<T extends /* c */ (A extends B ? C : D)> = T;
</script>
