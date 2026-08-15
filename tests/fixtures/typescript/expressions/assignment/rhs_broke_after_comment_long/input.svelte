<script lang="ts">
	// At exactly 100 wide the glued form fits and the authored newline collapses.
	const a1 = /* c */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa;

	// One char more breaks the `=` seam: the run holds its own line, the value re-fits below.
	const a2 =
		/* c */
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa;

	// The assignment expression shares the seam; the value group breaks by its own rules below.
	b1 =
		/* c */
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb +
		cccccccccccccccccccccccccccccccccccccccccccc +
		ddddd;
</script>
