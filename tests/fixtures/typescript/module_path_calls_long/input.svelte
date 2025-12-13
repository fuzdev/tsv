<script lang="ts">
	// #17882: Module path calls with long paths - prettier keeps them on one line

	// require() - stays on one line even when exceeding print width
	const a = require('./a/long/long/long/long/long/long/long/long/long/long/long/long/path/to/module');

	// require.resolve() - breaks at assignment, keeps call together
	const b =
		require.resolve('./a/long/long/long/long/long/long/long/long/long/long/long/path/to/module');

	// require.resolve.paths() - breaks at chain (.paths on new line)
	const c = require.resolve
		.paths('./a/long/long/long/long/long/long/long/long/long/long/long/path/to/module');

	// await import() - breaks at assignment, keeps call together
	const d =
		await import('./a/long/long/long/long/long/long/long/long/long/long/long/path/to/module');

	// import.meta.resolve() - breaks at chain (.resolve on new line)
	const e = import.meta
		.resolve('./a/long/long/long/long/long/long/long/long/long/long/long/path/to/module');

	// Short paths stay on one line
	const f = require('./short');
	const g = require.resolve('./short');
	const h = require.resolve.paths('./short');
	const i = await import('./short');
	const j = import.meta.resolve('./short');
</script>
