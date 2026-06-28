<script>
	// binary operand in an array spread — parens kept by both formatters
	const a = [...(x + y) /* c */];

	// binary operand in a call spread
	fn(...(x + y) /* c */);
</script>
