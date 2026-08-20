<script>
	// A blank line before a trailing `.prop` is NOT chain-expansion signal: prettier roots
	// the member chain at the outermost CALL, so this blank sits between the chain and the
	// `printMemberExpression` above it, which has no blank handling — the blank collapses
	// and the lookup lays out on width alone.

	// the chain fits, so it stays flat and the blank goes
	aa.bbbb().cccc().dddd()

		.eeee;

	// two tail members, same answer
	aa.bbbb().cccc()

		.dddd.eeee;

	// over width, so the tail takes the break point the peel gives it
	bbbbbbb()

		.dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd;

	// Contrast: a blank BETWEEN methods is inside the chain, and both formatters keep it
	aa.bbbb()

		.cccc()
		.dddd();
</script>
