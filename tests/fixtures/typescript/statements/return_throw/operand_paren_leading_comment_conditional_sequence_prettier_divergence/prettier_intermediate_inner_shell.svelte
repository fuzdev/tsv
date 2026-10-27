<script lang="ts">
	// The own-line comment sits inside the paren shell of the argument's LEFTMOST node — the
	// conditional's test, the sequence's head. The walk down the left side finds it, and the
	// hanging parens hold the argument to the keyword's line (a break after `return` / `throw`
	// is ASI). Both formatters settle here; prettier takes two passes from the authored form.
	function fn1() {
		return (
			// c
			(a as any)
				? b
				: c
		);
	}
	function fn2() {
		return (
			// c
			a,
			b
		);
	}

	// throw is restricted the same way
	function fn3() {
		throw (
			// c
			(a as any)
				? b
				: c
		);
	}

	// a comment below a pair the argument's left spine prints, below a non-null operand pair a
	// member chain flattens and a cast operand, still leads the argument inside the hanging parens
	function fn4() {
		return (
			(// c
			a * 2)!.b
		);
	}
	function fn5() {
		return (
			(
				// c
				a * 2
			).b satisfies T
		);
	}
</script>
