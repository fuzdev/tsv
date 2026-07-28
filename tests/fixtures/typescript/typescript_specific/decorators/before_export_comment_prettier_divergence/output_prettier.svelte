<script lang="ts">
	// a comment in the `export`→class gap of a decorator-first class stays where it is
	@dec // c1
	export class C {}

	@dec /* c2 */
	export class D {}

	// the gap sits inside the declaration, so an own-line directive there freezes nothing —
	// it only keeps its own line
	@dec
	// prettier-ignore
	export class E {}
</script>
