<script lang="ts">
	// Block then line comment between extends items
	interface A extends I /* c1 */,
		// c2
		J {}

	// Line then block comment between extends items
	interface B extends I,// c1
		/* c2 */
		J {}

	// Trailing block before the comma plus a same-line line comment: both trail the comma
	interface C extends I /* c1 */, // c2
		J {}

	// Own-line line comment after the comma sits on its own line below
	interface D extends I,
		// c1
		J {}
</script>
