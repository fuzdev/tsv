<script lang="ts">
	// A line comment after `extends` hangs the parenthesized conditional
	// constraint (clarity parens kept).
	function fn<
		T extends // c
			(A extends B ? C : D)
	>(): void {}

	// The same shell's TRAILING gap is lifted onto the end of the clarity parens. A
	// LINE comment there retains the shell instead — see
	// required_paren_shell_line_comment_prettier_divergence
	function fn2<
		T extends // c
			(A extends B ? C : D) /* t */
	>(): void {}

	// A same-line block comment stays inline before the parens.
	type E<T extends /* c */ (A extends B ? C : D)> = T;

	// Both of the shell's own gaps, around the parens the constraint re-emits
	type F<T extends /* c */ (A extends B ? C : D) /* t */> = T;
	type G<T extends (A extends B ? C : D) /* t */> = T;

	// A redundant double shell collapses to the one pair the constraint prints
	type H<T extends /* c */ (A extends B ? C : D)> = T;

	// Control: the `=` default position strips the parens, so its shell's comments
	// lead and trail the bare conditional
	type I<T = /* c */ A extends B ? C : D /* t */> = T;
</script>
