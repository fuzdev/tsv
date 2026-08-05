<script>
	// after-comma block glued to the last spread element
	const a = [1, ...x1, /* c1 */];

	// own-line comment from stripped parens, then an after-comma block glued to the spread
	const b = [2, ...(x2
	/* i */), /* c2 */];
</script>
