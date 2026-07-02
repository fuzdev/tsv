<script lang="ts">
	// await using in async function
	async function process() {
		await using resource = getAsyncResource();
		return resource.getValue();
	}

	// await using with type annotation
	async function typed() {
		await using resource: AsyncDisposable = getAsyncResource();
	}

	// Multiple await using declarators (separate statements)
	async function multi() {
		await using a = x;
		await using b = y;
	}

	// await using with null (still causes await on scope exit)
	async function optional() {
		await using nothing = null;
	}

	// Mixed using and await using
	async function mixed() {
		using sync = getSync();
		await using asyncRes = getAsync();
	}

	// await using in block scope inside async function
	async function scoped() {
		{
			await using inner = getResource();
		}
	}

	// await using in for-await-of loop
	async function iterate(resources: AsyncIterable<AsyncDisposable>) {
		for await (await using resource of resources) {
			console.log(resource.getValue());
		}
	}

	// Disposal order (reverse of declaration)
	async function disposal() {
		await using first = new Resource('first');
		await using second = new Resource('second');
	}

	// Contextual keywords as binding names — any identifier-shaped word that
	// is not an expression continuation (`in`/`instanceof`/`as`/`satisfies`)
	// binds, even when it lexes as a keyword
	async function keywords() {
		await using async = getAsyncResource();
		await using undefined = getAsyncResource();
		await using of = getAsyncResource();
	}
</script>
