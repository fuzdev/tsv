<script>
	function* g() {
		// A yield operand's chain takes a continuation indent, like every other nested
		// binary operand - a YieldExpression parent is in neither of prettier's exempt lists
		const a = yield bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
			cccccccccccccccccccccccccccccc ??
			ddd;

		// The delegating spelling takes the same shape
		const b = yield* bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
			cccccccccccccccccccccccccccccc ??
			ddd;

		// 100 chars - stays inline
		const c = yield bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ?? cccccccccccccccccccccccccccc ?? ddd;

		// 101 chars - the chain breaks
		const d = yield bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
			ccccccccccccccccccccccccccccc ??
			ddd;
	}
</script>
