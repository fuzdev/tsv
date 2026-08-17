<script>
	// line comment trailing a function body `{`
	function f1() { // c1
		body1();
	}

	// block comment trailing the body `{`, then own-line leading comment
	function f2() { /* brace */
		// lead
		body2();
	}

	// line comment trailing a plain block `{`
	{ // c3
		body3();
	}

	// line comment trailing an arrow block body `{`
	const g = () => { // c4
		body4();
	};

	// an author blank BELOW the pulled comment survives — the blank is authorship, not
	// the container's leading gap
	function f3() { // c5

		body5();
	}

	// the same, with the block spelling (an own-line comment is what pulls a block onto
	// the `{` line)
	function f4() { /* brace2 */

		// lead
		body6();
	}

	// the blank ABOVE the comment is a different question and stays erased — it sits
	// against the delimiter, where both formatters drop it at every bracket
	function f5() {
		// c6
		body7();
	}
</script>
