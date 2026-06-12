<script>
	// assignment trailing comment stays before the semicolon (matches prettier)
	a=(x/* c */);

	// leading comment preserved before the expression
	const b=(/* c */x);
</script>
