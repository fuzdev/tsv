<script lang="ts">
	// A block comment between an operand end and the `/` — the `/` still divides
	if (aa++ /* c1 */ / bb > 1) {
		g();
	}

	// Same after a call's `)` and an index's `]`
	if (fn() /* c2 */ / bb > 1) {
		g();
	}

	if (arr[0] /* c3 */ / bb > 1) {
		g();
	}

	// Same after a plain identifier
	if (aa /* c4 */ / bb > 1) {
		g();
	}

	// A binary `+` does NOT end an operand, so the `/` still opens a regex
	if (aa + /* c5 */ /re/.test(b)) {
		g();
	}
</script>
