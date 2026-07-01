<script lang="ts">
	// A single-line block comment in an indexed access's `[`→index gap collapses
	// inline; the own-line authoring (unformatted_ours_own_line) collapses to this.
	// Prettier relocates the comment out before `[` (`A /* c */[K]`).
	type X = A[/* c */
	K];
</script>
