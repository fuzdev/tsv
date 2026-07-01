<script lang="ts">
	// A single-line block comment after a type predicate's `is` collapses inline;
	// the own-line authoring (unformatted_ours_own_line) collapses to this. Prettier
	// instead relocates the comment before `is` (`x /* a */ is T`).
	function f(x): x is
		/* a */
		T {
		return true;
	}
</script>
