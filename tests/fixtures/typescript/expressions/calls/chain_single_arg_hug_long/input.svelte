<script>
	// A member call's lone huggable argument takes the same three states as a plain call's:
	// hugged flat, hugged with the argument broken, then the argument on its own line. An
	// argument with nothing to break inside it reaches the last state.

	// Boundary: exactly 100 chars - fits, the empty object stays hugged.
	obj.mAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA({});

	// Boundary: exactly 101 chars - nothing inside can break, so the argument takes its own line.
	obj.mAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA(
		{}
	);
	obj.mAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA(
		[]
	);
	obj.mAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA(
		function () {}
	);
	obj.mAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA(
		class {}
	);

	// Control: an argument that CAN break stays hugged and breaks inside the parens.
	obj.mAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA({
		aaaaaaaaaaaa: bbbbbbbbbbbb,
		cccccccccccc: dddd
	});

	// Plain identifier parameters render flat inside the hug, so the argument breaks out.
	obj.mAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA(
		function named(paramOne, paramTwo, paramThree, paramFour) {
			s();
		}
	);

	// A default value leaves the list breakable, so the same width hugs and breaks it.
	obj.mAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA(function named(
		paramOne,
		paramTwo = 1,
		paramThree,
		four
	) {
		s();
	});
</script>
