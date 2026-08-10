<script>
	// an own-line comment in a stripped paren shell leads the element after the comma
	const { /* c */ a1 = 1, b1 } = x;

	// the array pattern's spelling
	const [/* c */ a2 = 1, b2] = x;

	// the parameter list's spelling
	function fn(/* c */ a3 = 1, b3) {}
</script>
