<script lang="ts">
	// Block comment between `await` and `using` — stays where the author wrote it
	async function single() {
		await using /* c1 */ a = f();
	}

	// Comments on both sides of `using` stay distinct and in order
	async function both() {
		await using /* c2 */ /* c3 */ b = f();
	}

	// The same gap in a for-await-of head
	async function iterate(it: AsyncIterable<AsyncDisposable>) {
		for await (await using /* c4 */ c of it) {
		}
	}
</script>
