<script>
	// When a member chain's argument layout expands, a block comment leading the first
	// argument leads that argument: it stays with the argument rather than riding the
	// `(` line, and it breaks the argument out of the hug a wide literal would take.

	// Control: nothing expands, so the comment stays inline with the argument.
	const a1 = obj.fn1().fn2(/* c */ { prop1: 'value1' });

	// A sole object argument wide enough to break: the comment leads it on its own line.
	const a2 = obj
		.fn1()
		.fn2(
			/* c */ {
				prop1: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
			}
		);

	// The array twin.
	const a3 = obj
		.fn1()
		.fn2(
			/* c */ [
				'value1value1value1value1',
				'value2value2value2value2',
				'value3value3value3value3',
				'value4value4value4value4'
			]
		);

	// A function-composition argument list forces expansion: the comment leads the first
	// argument rather than riding the `(` line.
	const a4 = obj.fn1().fn2(
		/* c */ arr.map((x) => x),
		b
	);
</script>
