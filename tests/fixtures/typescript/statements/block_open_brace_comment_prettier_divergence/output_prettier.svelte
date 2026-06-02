<script>
	// line comment trailing a function body `{`
	function f1() {
		// c1
		body1();
	}

	// block comment trailing the body `{`, then own-line leading comment
	function f2() {
		/* brace */
		// lead
		body2();
	}

	// line comment trailing a plain block `{`
	{
		// c3
		body3();
	}

	// line comment trailing an arrow block body `{`
	const g = () => {
		// c4
		body4();
	};
</script>
