<script lang="ts">
	// A single-line block comment after a heritage keyword (`extends`/`implements`)
	// collapses inline; the own-line authoring (unformatted_ours_own_line) collapses
	// to this. Prettier keeps it on its own line before the keyword (it relocates the
	// comment across the keyword but, unlike the other gaps, does not inline it).
	class P
		/* a */
		extends Q {}

	class R
		/* b */
		implements S {}

	interface T
		/* c */
		extends U {}
</script>
