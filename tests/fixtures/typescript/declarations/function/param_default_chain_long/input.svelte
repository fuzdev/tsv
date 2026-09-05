<script>
	// A default parameter value's chain takes a continuation indent: prettier's
	// shouldIndentIfInlining does NOT list AssignmentPattern, so the chain arm fires
	function dp(
		p = bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
			cccccccccccccccccccccccccccccccccccccccc ??
			ddd
	) {}

	// The inlining spelling takes the same shape - the position is not an assignment value
	function dq(
		p = bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
			cccccccccccccccccccccccccccccccccccccccc ?? [1]
	) {}

	// 100 chars - stays inline
	function dr(p = bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ?? ccccccccccccccccccccccccccc ?? ddd) {}

	// 101 chars - the parameter list breaks
	function ds(
		p = bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ?? cccccccccccccccccccccccccccc ?? ddd
	) {}
</script>
