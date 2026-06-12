<script>
	const a = import(/* @vite-ignore */expr);

	const b = await import(/* @vite-ignore */'./a');

	const c = import(/* webpackChunkName: "chunk" */'./a');

	const d = import(/* comment1 */ /* comment2 */expr);
</script>
