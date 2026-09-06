<script lang="ts">
	// The own-line comment sits inside the paren shell of the argument's LEFTMOST node rather
	// than ahead of the whole argument (`(⏎// c⏎a as any).b`). The walk down the left side
	// finds it, and the hanging parens hold the argument to the keyword's line — a break after
	// `return` / `throw` is ASI, so the bare form would change the program.
	function fn1() {
		return (
			// c
			a as any
		).b;
	}

	// the same at every left-side kind whose shell strips: callee, tag, binary operand,
	// assignment target, non-null operand
	function fn2() {
		return (
			// c
			a
		)();
	}
	function fn3() {
		return (
			// c
			a
		)``;
	}
	function fn4() {
		return (
			// c
			a as any
		) + b;
	}
	function fn5() {
		return (
			// c
			a
		) = b;
	}
	function fn6() {
		return (
			// c
			a
		)!;
	}

	// throw is restricted the same way
	function fn7() {
		throw (
			// c
			a as any
		).b;
	}
</script>
