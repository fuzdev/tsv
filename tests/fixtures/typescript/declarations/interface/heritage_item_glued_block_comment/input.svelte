<script lang="ts">
	// A block glued to the next heritage item leads it, even when a line comment shares the gap.
	interface Single
		extends
			Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, // line
			/* block */ Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb {}

	// Blocks glued to each other but not to the item stay together on their own line.
	interface Multiple
		extends
			Cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc,
			/* block1 */ /* block2 */
			Dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd {}
</script>
