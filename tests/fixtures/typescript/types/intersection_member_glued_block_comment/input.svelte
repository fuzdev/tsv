<script lang="ts">
	// Blocks glued to each other but not to the intersection member stay together on their own line.
	type A = Cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc &
		/* block1 */ /* block2 */
		Dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd;

	// The run keeps its line at every boundary, not just the first, and the members below it
	// still break on their own.
	type B = Cccccccccccccccccccccccccccccccccccccccccccccccccccccccc &
		/* block1 */ /* block2 */
		Ddddddddddddddddddddddddddddddddddddddddddddd &
		Eeeeeeeeeeeeeeeeeeeeeee;

	// The same run collapses inline once the intersection fits — the line after it is the
	// group's to decide.
	type C = X & /* block1 */ /* block2 */ Y;
</script>
