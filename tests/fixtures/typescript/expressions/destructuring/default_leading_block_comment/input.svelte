<script>
	// a block comment glued to a defaulted element binds to that element, once
	const [/* c */ a1 = 1] = x;

	// the same at a non-first element
	const [a2, /* c */ b2 = 1] = x;

	// a nested pattern as the default's left side
	const [/* c */ [a3] = 1] = x;

	// the array pattern in a parameter position
	function fn([/* c */ a4 = 1]) {}

	// the object pattern's twin, which binds it the same way
	const { /* c */ a5 = 1 } = x;
</script>
