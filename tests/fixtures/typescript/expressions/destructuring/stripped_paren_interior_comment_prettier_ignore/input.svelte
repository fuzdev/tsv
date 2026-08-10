<script>
	// a frozen property keeps its shell verbatim, and the comment inside it once
	const {
		// prettier-ignore
		a1  =  (1 /* c */),
		b1
	} = x;

	// the array pattern's spelling
	const [
		// prettier-ignore
		a2  =  (1 /* c */),
		b2
	] = x;

	// the parameter list's spelling
	function fn(
		// prettier-ignore
		a3  =  (1 /* c */),
		b3
	) {}
</script>
