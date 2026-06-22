<script lang="ts">
	async function fn1() {
		// Chain .then() callback - break line exactly 100 chars
		const a: Record<string, Record<string, string[]>> = await fn123('/aaa/bbbbbbbbbbbb').then((r) =>
			r.json()
		);

		// Chain .then() callback - break line 101 chars (args broken out)
		const b: Record<string, Record<string, string[]>> = await fn123('/aaa/bbbbbbbbbbbbb').then(
			(r) => r.json()
		);
	}
</script>
