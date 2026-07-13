<script lang="ts">
	@a
	/* c1 */
	@b
	class A {}

	@a
	// c2
	@b
	class B {}

	@a
	/* c3 */ @b
	class C {}

	@a
	/* c4 */
	class D {}

	@a
	// c5
	class E {}
</script>
