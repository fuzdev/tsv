<script>
	// Two empty-body callbacks on an identifier head stay inline
	promise.then(() => {}).catch(() => {});

	// Assignment and declaration right-hand sides
	result = promise.then(() => {}).catch((e) => {});
	const ids = items.filter(function (d) {}).map((d) => d.id);

	// Awaited right-hand side
	async function load() {
		const x = await promise.then(() => {}).catch(() => {});
	}

	// Member head
	this.promise.then(async () => {}).catch(() => {});

	// Call head breaks at two calls regardless
	fetch(url)
		.then(() => {})
		.catch(() => {});

	// Short head merges the first call
	p.then(() => {}).catch(() => {});

	// Comment-only body breaks the chain
	promise
		.then(() => {
			// todo
		})
		.catch(() => {});

	// Block-comment body breaks the chain
	promise
		.then(() => {
			/* noop */
		})
		.catch(() => {});

	// Non-empty body breaks the chain
	promise
		.then(() => {
			done();
		})
		.catch(() => {});
</script>

<button onclick={() => promise.then(() => {}).catch(() => {})}>x</button>

{#each items.filter(() => {}).map(() => {}) as item}{item}{/each}
