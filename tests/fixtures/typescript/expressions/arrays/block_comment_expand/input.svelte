<script lang="ts">
	// Multi-line block comment at start of array forces expansion
	const a = [
		/* comment line 1
comment line 2 */ b,
		c,
		d
	];

	// Multi-line block comment in middle also forces expansion
	const e = [
		f,
		/* comment line 1
comment line 2 */ g,
		h
	];

	// Single-line block comment does NOT force expansion (short array)
	const i = [/* comment */ j, k, l];
</script>
