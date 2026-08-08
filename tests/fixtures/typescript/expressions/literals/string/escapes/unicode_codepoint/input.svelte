<script>
	const letter = '\u{41}';
	const emoji = '\u{1F4A9}';
	const max = '\u{10FFFF}';
	// Leading zeros are unbounded: the spec caps the VALUE (<= 0x10FFFF), not the digit count
	const padded = '\u{0000041}';
	const manyZeros = '\u{0000000000000041}';
	const paddedMax = '\u{00010FFFF}';
</script>
