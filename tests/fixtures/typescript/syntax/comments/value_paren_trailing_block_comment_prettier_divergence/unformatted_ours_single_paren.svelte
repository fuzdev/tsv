<script>
	// A redundant paren shell around a value whose trailing gap holds a block comment.
	// A value position is `;`-terminated, so the comment defers past the `;` — the
	// declarator's own value-to-`;` handling — from either authoring, in one pass.
	const a = (x /* t */);

	const b = (fn() /* t */);

	// Assignment RHS and compound assignment take the same answer.
	c = (x /* t */);

	d += (x /* t */);

	// A `return` argument is a value position too.
	function fn1() {
		return (x /* t */);
	}

	// A `throw` argument answers the same, including when the operand takes clarity
	// parens: those close before the gap, so they hold nothing inside.
	function fn2() {
		throw (c = d /* t */);
	}

	// `export default` too — and its own clarity pair closes at the expression, so it
	// never encloses this gap the way a `return` argument's retained parens do.
	export default (e = f /* t */);
</script>
