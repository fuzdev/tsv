<script>
	async function fn() {
		// await over assignment operators — parens are structurally required
		// Without parens, `await b ??= c()` parses as `(await b) ??= c()` which is invalid
		const a = await (b ??= c());
		const d = await (e ||= f());
		const g = await (h &&= i());
		const j = await (k = l());
		const m = await (n += 1);

		// yield has the same issue
		function* gen() {
			const o = yield (p ??= q());
			const r = yield (s = t());
		}
	}
</script>
