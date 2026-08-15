<script>
	// A redundant paren shell around a value whose trailing gap holds a block comment.
	// A value position is `;`-terminated, so the comment defers past the `;` — the
	// declarator's own value-to-`;` handling — from either authoring, in one pass.
	const a = x; /* t */

	const b = fn(); /* t */

	// Assignment RHS and compound assignment take the same answer.
	c = x; /* t */

	d += x; /* t */

	// A `return` argument is a value position too.
	function fn1() {
		return x; /* t */
	}
</script>
