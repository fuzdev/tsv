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

	// Trailing block before the comma plus a same-line line comment: both trail the comma
	class D
		implements
			I /* c1 */, // c2
			J {}

	// Own-line line comment after the comma sits on its own line below
	class E
		implements
			I,
			// c1
			J {}
</script>
