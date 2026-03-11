<script lang="ts">
	async function fn() {
		// Trailing chain at 100 chars - stays on same line
		const g = (
			await fn({
				...o,
				a: bbbbbbb,
			})
		).prop?.method_cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc();

		// Trailing chain at 101 chars - both formatters keep on one line
		const h = (
			await fn({
				...o,
				a: bbbbbbb,
			})
		).prop?.method_ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc();

		// Chain tail with second method at 100 chars - prettier breaks multi-call chains
		const i = (
			await fn({
				...o,
				a: bbbbbbb,
			})
		).prop
			?.method_ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc()
			.method_d();

		// Chain tail with second method at 101 chars - prettier breaks multi-call chains
		const j = (
			await fn({
				...o,
				a: bbbbbbb,
			})
		).prop
			?.method_cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc()
			.method_d();
	}
</script>
