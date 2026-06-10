<script>
	// `// prettier-ignore` on a member access prints the whole member expression
	// verbatim from source; inner call args (numbers) are preserved, not reformatted.

	const a = obj.
		// prettier-ignore
		prop;

	const b = fn(2.0000)
		// prettier-ignore
		.c;

	obj.method(2.0000)
		// prettier-ignore
		.chained;

	const d = obj?.
		// prettier-ignore
		prop;
</script>
