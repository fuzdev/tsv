<script lang="ts">
	// A single-line block comment after `as`/`satisfies` collapses inline; the
	// own-line authoring (unformatted_ours_own_line) collapses to this. Prettier
	// relocates the comment before the keyword (`x /* a */ as A`).
	const a = x as
		/* a */
		A;

	const b = x satisfies
		/* b */
		B;
</script>
