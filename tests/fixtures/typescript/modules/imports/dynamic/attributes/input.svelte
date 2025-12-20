<script lang="ts">
	// Basic with string literal
	const a = await import('./a', {with: {type: 'json'}});

	// Variable as source
	const path = './b';
	const b = await import(path, {with: {type: 'json'}});

	// Chained .then()
	import('./c', {with: {type: 'json'}}).then((m) => m.init());

	// Destructuring result
	const {default: c} = await import('./d', {with: {type: 'json'}});

	// In async function
	async function fn() {
		const d = await import('./e', {with: {type: 'json'}});
		return d;
	}

	// Expression as source
	function getPath() {
		return './f';
	}
	const e = await import(getPath(), {with: {type: 'json'}});

	// Template literal source
	const name = 'g';
	const f = await import(`./${name}`, {with: {type: 'json'}});

	// Optional chaining on result
	const g = (await import('./h', {with: {type: 'json'}}))?.default;

	// Multiple attributes
	const h = await import('./i', {with: {type: 'json', resolution: 'import'}});

	// Nested import with options (long - wraps)
	const i = await import(await import('./path', {with: {type: 'json'}}).then((m) => m.path), {
		with: {type: 'json'},
	});
</script>
