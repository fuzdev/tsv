<script lang="ts">
	// Unicode escapes: \u0066oo → foo, \u{41} → A
	const \u0066oo = 1;
	const b\u0061r = 2;
	const \u{41} = 3;

	// The spec caps a `\u{}` escape's VALUE, not its digit count, so leading
	// zeros are unbounded: \u{0000000000000042} → B
	const \u{0000000000000042} = 4;
</script>
