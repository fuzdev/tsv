<script lang="ts">
	// an own-line block before a member the chain prints inline is glued, so no pair is owed
	function fn1() {
		return a
		/* c */
		.b;
	}
	function fn2() {
		throw (a)
		/* c */
		?.b;
	}
	function* fn3() {
		yield a
		/* c */
		.b + 1;
	}

	// a multi-line block glues the same way
	function fn4() {
		return a
		/* m
		n */
		.b;
	}

	// an assignment argument keeps only its clarity pair
	function fn5() {
		return a
		/* c */
		.b = 1;
	}

	// a member the chain's head absorbs, and a member-only chain broken for width
	function fn6() {
		return a
		/* c */
		.b.m(1);
	}
	function fn7() {
		return a
		/* c */
		.bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb.cccccccccccccccccccccccccccccccccccccccccc.dddddddddddddddddd;
	}

	// a block on a kept pair's `)` line stays there, below the pair's `//`
	function fn8() {
		return (a?.() // c
		) /* d */ .m();
	}

	// a comment the chain prints on a line of its own keeps the pair: before a call it breaks
	// before, a line comment, and a computed member
	function fn9() {
		return a
		/* c */
		.b();
	}
	function fn10() {
		return a
		// c
		.b;
	}
	function fn11() {
		return a
		/* c */
		[k];
	}

	// the same read at a private member, at a called member behind a multi-line block's closing
	// line the chain keeps glued, and through an assignment argument
	class K {
		#p = 1;
		fn12() {
			return a
			/* c */
			.#p;
		}
	}
	function fn13() {
		return a
			.b()
			.c /* m
		n */ /* c */.d(1);
	}
	function fn14() {
		return a
		// c
		.b = 1;
	}
</script>
