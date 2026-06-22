<script lang="ts">
	// Array with 2+ objects having 2+ properties triggers break_parent
	// Prettier keeps first arg inline, only expands array
	fn('x', [
		{ a: 1, b: 2 },
		{ c: 3, d: 4 }
	]);

	// More args - same pattern
	fn('a', 'b', [
		{ x: 1, y: 2 },
		{ z: 3, w: 4 }
	]);

	// With call expression as first arg
	assert.deepEqual(parse('/foo'), [
		{ type: 'a', path: '/' },
		{ type: 'b', name: 'foo' }
	]);
</script>
