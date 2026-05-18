<script lang="ts">
	// Block then line comment between extends items
	interface A
		extends
			I /* c1 */,
			// c2
			J {}

	// Line then block comment between extends items
	interface B
		extends
			I, // c1
			/* c2 */
			J {}
</script>
