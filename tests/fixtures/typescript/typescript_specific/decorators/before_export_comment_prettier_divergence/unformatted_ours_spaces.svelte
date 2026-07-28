<script lang="ts">
	// a comment in the `export`→class gap of a decorator-first class stays where it is
	@dec
	export // c1
		class   C   {}

	@dec
	export /* c2 */ class   D   {}

	// the gap sits inside the declaration, so an own-line directive there freezes nothing —
	// it only keeps its own line
	@dec
	export
		// prettier-ignore
		class   E   {}
</script>
