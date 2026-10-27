<script lang="ts">
	// a comment glued to a mid-chain shell's `(` trails the chain's head on the keyword's line
	function fn1() {
		return a // d
			.b()
			.c.m();
	}

	// one on a line of its own below the `(` hangs the argument
	function fn2() {
		return (
			a
				// c
				.b.m(1)
		);
	}

	// nested shells, each `//` glued to its own `(`: the second prints below the first
	function fn3() {
		return (
			a // d
				// e
				.b.m()
		);
	}

	// a line the source opens before a `(`, ended by a `//` below it, hangs the argument
	function fn4() {
		return (
			a /* d */
				// e
				.b.m()
		);
	}

	// a comment on a multi-line block's closing line in a called member's gap hangs the argument
	function fn5() {
		return (
			/* e */ a /*
			 */.b.c /*
				 */ /* e */
				.m()
		);
	}
	function fn6() {
		return (
			a.b.c /*
				 */ /* e */
				.m()
		);
	}
</script>
