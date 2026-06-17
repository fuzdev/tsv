<script>
	// Trailing line comment on test - binary test fits, comment pushes line over 100 chars
	const x =
		aaaaa === bbbbb // comment that pushes the test line well over the print width xxxxxxxxxxxxxxxxxx
			? null
			: fn(value);
</script>
