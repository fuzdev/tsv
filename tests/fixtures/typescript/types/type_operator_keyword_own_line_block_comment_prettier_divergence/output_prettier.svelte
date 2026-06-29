<script lang="ts">
	// Own-line block comment after a prefix type operator, before the operand:
	// kept on its own line with the operand hanging indented one level (the shared
	// keyword→value layout).
	type A = keyof /* a */
	B;

	// typeof (a TypeQuery node) hangs the same way.
	type B = typeof /* b */
	x;

	// readonly (a TypeOperator, like keyof) hangs the same way.
	type C = readonly /* c */
	string[];

	// A blank line after the comment is preserved.
	type D = keyof /* d */

	B;

	// A same-line block comment with the operand authored on the next line keeps
	// the comment trailing the operator and the operand hanging.
	type E = keyof /* e */ B;

	// A same-line block comment glued to the operand stays fully inline.
	type F = keyof /* f */ B;
</script>
