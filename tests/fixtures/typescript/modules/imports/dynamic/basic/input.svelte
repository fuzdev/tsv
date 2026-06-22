<script lang="ts">
	// Basic dynamic import
	const module = import('module');

	// With await
	const data = await import('./data.json');

	// Variable as source
	const path = './module';
	const mod = import(path);

	// In async function
	async function load() {
		const { default: fn } = await import('./fn');
		return fn;
	}

	// Chained
	import('./mod').then((m) => m.init());

	// Nested
	const nested = await import(await import('./path').then((m) => m.path));
</script>
