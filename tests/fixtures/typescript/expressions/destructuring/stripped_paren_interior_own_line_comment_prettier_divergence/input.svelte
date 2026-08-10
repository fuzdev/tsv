<script>
	// an own-line comment in a stripped paren shell leads the element after the comma
	const { a1 = 1, /* c */ b1 } = x;

	// the array pattern's spelling
	const [a2 = 1, /* c */ b2] = x;

	// the parameter list's spelling
	function fn(a3 = 1, /* c */ b3) {}
</script>
