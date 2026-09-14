<script lang="ts">
	// Block comment after `?`: tsv keeps it after the operator, where it was written.
	type A = B extends C ? /* c1 */
		D : E;

	// Block comment after `:`: same rule, the other slot.
	type F = B extends C ? D : /* c2 */
		E;

	// A run holding a multi-line block: the run's own forced break keeps the author's
	// break after it, so the run takes the `?` line and the value the one below.
	type G = B extends C ? /* c3
		d3 */
		/* c4 */ D : E;
</script>
