<script>
	// A bare expression statement's chain takes a continuation indent - an
	// ExpressionStatement parent is in neither of prettier's exempt lists
	bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
		cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc ??
		ddd;

	// A labeled statement wraps an ordinary expression statement, so it takes the same shape
	lab: bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
		ccccccccccccccccccccccccccccccccccccccccccccccccccccccc ??
		ddd;

	// 100 chars - stays inline
	bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ?? cccccccccccccccccccccccccccccccccccccccccccccc ?? ddd;

	// 101 chars - the chain breaks
	bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
		ccccccccccccccccccccccccccccccccccccccccccccccc ??
		ddd;
</script>
