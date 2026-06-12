<script lang="ts">
	// Block then line comment between implements items
	class A
		implements
			I /* c1 */,
			// c2
			J {}

	// Line then block comment between implements items
	class B
		implements
			I, // c1
			/* c2 */
			J {}

	// Two block comments between implements items
	class C implements I /* c1 */ /* c2 */, J {}
</script>
