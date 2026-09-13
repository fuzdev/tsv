<script lang="ts">
	// The block comment SPANS A LINE inside the paren shell of the argument's leftmost node
	// (`(/* a⏎b */ a).b`), so the run the shell's strip would hoist lands between the keyword
	// and its argument: a MultiLineComment holding a line terminator IS one for ASI (ECMA-262
	// §12.4), and `return` / `throw` are restricted productions. The hanging parens hold it.
	function fn1() {
		return (/* a
b */ a).b;
	}

	// every left-side kind whose shell strips takes the same pair: callee, template tag,
	// non-null operand, conditional test, a longer chain
	function fn2() {
		return (/* a
b */ a)();
	}
	function fn3() {
		return (/* a
b */ a)`t`;
	}
	function fn4() {
		return (/* a
b */ a)!;
	}
	function fn5() {
		return (/* a
b */ a) ? b : c;
	}
	function fn6() {
		return (/* a
b */ a).b.c();
	}

	// a binary argument reaches the same pair through its own conditional parens
	function fn9() {
		return (/* a
b */ a) + b;
	}

	// throw is restricted the same way
	function fn10() {
		throw (
			/* a
b */ a.b
		);
	}
</script>
