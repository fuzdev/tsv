<script lang="ts">
	async function fn() {
		// Parenthesized await with member chain: (await call({...spread})).member?.chain()
		// When exceeding print width, break after `= (` and keep object compact

		// 100 chars total - no wrap needed
		const a = (await fn({ ...o, a: bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb })).prop?.method();

		// 101 chars total - breaks after `= (`, object stays compact
		const b = (
			await fn({ ...o, a: bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb })
		).prop?.method();

		// Inner await line at 100 chars - object stays compact
		const c = (
			await fn({ ...o, a: bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb })
		).prop?.method();

		// Inner await line at 101 chars - object expands
		const d = (
			await fn({
				...o,
				a: bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
			})
		).prop?.method();

		// Property value at 100 chars - stays on single line within expanded object
		const e = (
			await fn({
				...o,
				a: bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
			})
		).prop?.method();

		// Property value at 101 chars - still single line (object already expanded, no further break)
		const f = (
			await fn({
				...o,
				a: bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
			})
		).prop?.method();
	}
</script>
