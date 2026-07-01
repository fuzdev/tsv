<script lang="ts">
	// A single-line block comment after a heritage keyword (`extends`/`implements`)
	// collapses inline; the own-line authoring (unformatted_ours_own_line) collapses
	// to this. Prettier relocates the comment before the keyword.
	class P extends
		/* a */
		Q {}

	class R implements
		/* b */
		S {}

	interface T extends
		/* c */
		U {}
</script>
