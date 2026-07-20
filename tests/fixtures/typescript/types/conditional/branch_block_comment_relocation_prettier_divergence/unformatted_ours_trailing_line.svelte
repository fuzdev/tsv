<script lang="ts">
	// Block comment after `?`: tsv keeps it after the operator, where it was written.
	type A = B extends C ? /* c1 */
		D : E;

	// Block comment after `:`: same rule, the other slot.
	type F = B extends C ? D : /* c2 */
		E;
</script>
