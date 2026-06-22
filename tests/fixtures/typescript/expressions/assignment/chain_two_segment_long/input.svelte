<script>
	// 2-segment chain with call - fits inline (100 chars)
	a = b = fn(ccccccccccccccccccccccccccccccccccccccccccc, dddddddddddddddddddddddddddddddddddddddd);

	// 2-segment chain with call - expands args (101 chars)
	a = b = fn(
		cccccccccccccccccccccccccccccccccccccccccccc,
		dddddddddddddddddddddddddddddddddddddddd
	);

	// 2-segment chain with array - expands elements
	a = b = [
		ccccccccccccccccccccc,
		ddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd
	];

	// 2-segment chain with new - expands args
	a = b = new Fn(
		cccccccccccccccccccccccccc,
		ddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd
	);

	// 2-segment chain with binary - break after operator
	a = b =
		cccccccccccccccccc + ddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd;

	// 2-segment chain with member chain
	a = b =
		cccccccccccccccccc.dddddddddddddddddddddddddddd.eeeeeeeeeeeeeeeeeeeeeee.ffffffffffffffffffff;
</script>
