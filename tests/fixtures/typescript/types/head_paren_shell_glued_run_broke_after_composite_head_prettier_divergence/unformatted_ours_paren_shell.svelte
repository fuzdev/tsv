<script lang="ts">
	// A run the author GLUED — a multi-line block ahead of a single-line one — and then
	// broke after, inside a redundant paren shell at a COMPOSITE head: the shell strips and
	// the run lays out as the shell-free authoring does, the value dropping below the run's
	// closing `*/`. Prettier is non-idempotent on the shell authoring at these two seams.

	// conditional check type
	type A = ( /* c1
	d1 */ /* c2 */
	B) extends C ? D : E;

	// first intersection member
	type F = ( /* c3
	d3 */ /* c4 */
	G) & H;
</script>
