<script>
	// Basic for-await-of
	async function process(stream) {
		for await (const chunk of stream) {
			console.log(chunk);
		}
	}

	// For-await with destructuring
	async function processEvents(events) {
		for await (const {type, data} of events) {
			handle(type, data);
		}
	}

	// For-await with let
	async function iterate(gen) {
		for await (let item of gen) {
			item = transform(item);
			use(item);
		}
	}
</script>
