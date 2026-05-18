<script>
	// Line comment then hole at start of multiline array
	const a = [
		,
		// c1
		1,
	];

	// Line comment then hole in middle of multiline array
	const b = [
		1,
		,
		// c2
		2,
	];

	// Multi-line block comment then hole
	const c = [
		,
		/*
	  c3
	*/
		1,
	];

	// Multi-line block comment then hole in middle
	const d = [
		1,
		,
		/*
	  c4
	*/
		2,
	];

	// Line comment after hole
	const e = [
		,
		// c5
		1,
	];
</script>
