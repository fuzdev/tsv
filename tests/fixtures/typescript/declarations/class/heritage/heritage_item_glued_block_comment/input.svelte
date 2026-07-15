<script lang="ts">
	// A block glued to the next heritage item leads it, even when a line comment shares the gap.
	class Single
		implements
			Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, // line
			/* block */ Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb {}

	// Blocks glued to each other but not to the item stay together on their own line.
	class Multiple
		implements
			Cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc,
			/* block1 */ /* block2 */
			Ddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd {}
</script>
