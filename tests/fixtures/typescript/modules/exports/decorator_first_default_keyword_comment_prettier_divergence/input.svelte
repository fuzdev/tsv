<script lang="ts">
	// Block comments inside the `export default` keyword when the decorators precede
	// `export` — the class stays a *declaration*, so this is a third printer path
	@dec
	export /* c1 */ default /* c2 */ class A {}
</script>
