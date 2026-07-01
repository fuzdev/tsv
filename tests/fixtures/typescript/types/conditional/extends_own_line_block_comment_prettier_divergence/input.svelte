<script lang="ts">
	// A single-line block comment after a conditional type's `extends` collapses
	// inline (the conditional stays on one line); the own-line authoring
	// (unformatted_ours_own_line) collapses to this. Prettier relocates the comment
	// before `extends` (`X /* a */ extends Y`).
	type A = X extends /* a */ Y ? T : F;
</script>
