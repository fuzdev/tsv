<script lang="ts">
	/* require() - 98 content chars (visual 100, doesn't wrap) */
	const a = require('./aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa');

	/* require() - 99 content chars (visual 101, we wrap, prettier keeps inline) */
	const b = require(
		'./aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
	);

	/* require.resolve() - breaks at assignment (both match) */
	const c =
		require.resolve('./aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa');

	/* require.resolve.paths() - we expand args, prettier breaks at chain */
	const d = require.resolve.paths(
		'./aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
	);

	/* await import() - breaks at assignment (both match) */
	const e =
		await import('./aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa');

	/* import.meta.resolve() - we expand args, prettier breaks at chain */
	const f = import.meta.resolve(
		'./aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
	);
</script>
