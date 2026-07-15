<script lang="ts">
	// Block comments inside the `export default` keyword when the decorators precede
	// `export` — the class stays a *declaration*, so this is a third printer path
	@dec /* c1 */ /* c2 */
	export default class A {}
</script>
