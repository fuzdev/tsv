<script lang="ts">
	// 100 chars - fits on one line
	await expect(fn([], 3, async () => {}, aaaaaaaaaaaaaaa)).rejects.toBe('bbbbbbbbbbbbbbbbbbbbbbbb');

	// 101 chars - prettier breaks at .toBe(), keeps inner call inline
	await expect(fn([], 3, async () => {}, aaaaaaaaaaaaaaa)).rejects.toBe(
		'bbbbbbbbbbbbbbbbbbbbbbbbb'
	);

	// Short - fits trivially
	await expect(fn(a, b)).rejects.toBe('c');
</script>
