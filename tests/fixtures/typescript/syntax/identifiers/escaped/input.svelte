<script lang="ts">
	// Unicode escapes: \u0066oo → foo, \u{41} → A
	const foo = 1;
	const bar = 2;
	const A = 3;

	// The spec caps a `\u{}` escape's VALUE, not its digit count, so leading
	// zeros are unbounded: \u{0000000000000042} → B
	const B = 4;
</script>
