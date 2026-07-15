<script lang="ts">
	// Block comment between `await` and `using` — stays where the author wrote it
	async function single() {
		await /* c1 */ using a = f();
	}

	// Comments on both sides of `using` stay distinct and in order
	async function both() {
		await /* c2 */ using /* c3 */ b = f();
	}

	// The same gap in a for-await-of head
	async function iterate(it: AsyncIterable<AsyncDisposable>) {
		for await (await /* c4 */ using c of it) {
		}
	}
</script>
