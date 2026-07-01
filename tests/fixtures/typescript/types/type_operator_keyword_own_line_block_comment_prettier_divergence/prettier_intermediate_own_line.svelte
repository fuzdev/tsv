<script lang="ts">
	// A single-line block comment between a prefix type operator and its operand is
	// inline; the own-line authoring (unformatted_ours_own_line) collapses to this.

	// keyof
	type A = keyof /* a */
	B;

	// typeof (a TypeQuery node)
	type B = typeof /* b */
	x;

	// readonly
	type C = readonly /* c */
	string[];
</script>
